import { useCallback, useEffect, useMemo, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "./api";
import { AuthScreen } from "./components/auth-screen";
import { Sidebar } from "./components/Sidebar";
import { ChatHeader } from "./components/ChatHeader";
import { Composer } from "./components/Composer";
import { Messages } from "./components/Messages";
import { EmptyState } from "./components/EmptyState";
import { SettingsView } from "./components/SettingsView";
import { MarketplaceView } from "./components/Marketplace";
import { TitleBar, type AppView } from "./components/TitleBar";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "./components/ui/resizable";
import { ExplorerPanel } from "./components/explorer/ExplorerPanel";
import type {
  ActiveProjectState,
  ChatMessage,
  ModelEntry,
  ProjectInfo,
  SlashCommandEntry,
  TranscriptSummary,
  UserProfile,
} from "./types";

type AuthState =
  | { kind: "loading" }
  | { kind: "auth"; profile: UserProfile; remembered: string | null }
  | { kind: "ready"; profile: UserProfile };

export function App() {
  const [auth, setAuth] = useState<AuthState>({ kind: "loading" });
  const [view, setView] = useState<AppView>("chat");
  const [models, setModels] = useState<ModelEntry[]>([]);
  const [activeModel, setActiveModel] = useState<string>("");
  const [transcripts, setTranscripts] = useState<TranscriptSummary[]>([]);
  const [activeSession, setActiveSession] = useState<string | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [busy, setBusy] = useState(false);
  const [slashCommands, setSlashCommands] = useState<SlashCommandEntry[]>([]);
  const [activeProject, setActiveProject] = useState<ProjectInfo | null>(null);
  const [explorerOpen, setExplorerOpen] = useState<boolean>(() => {
    try {
      return typeof window !== "undefined" && localStorage.getItem("vibn:explorerOpen") === "1";
    } catch {
      return false;
    }
  });

  const onActiveProjectChange = useCallback((state: ActiveProjectState) => {
    setActiveProject(state.active);
  }, []);

  const toggleExplorer = useCallback(() => {
    setExplorerOpen((v) => {
      const next = !v;
      try {
        localStorage.setItem("vibn:explorerOpen", next ? "1" : "0");
      } catch {
        /* ignore */
      }
      return next;
    });
  }, []);

  // Poll agent-emitted editor events (open_in_editor / show_in_explorer)
  // and re-fan them as DOM CustomEvents the explorer panel listens for.
  useEffect(() => {
    if (auth.kind !== "ready") return;
    let cancelled = false;
    const tick = async () => {
      if (cancelled) return;
      try {
        const events = await api.drainEditorEvents();
        for (const ev of events) {
          window.dispatchEvent(
            new CustomEvent(`vibn://${ev.kind.replace(/_/g, "-")}`, { detail: ev.payload }),
          );
          if (ev.kind === "open_in_editor") {
            // Surface the panel if it's closed so the user sees the file.
            const detail = ev.payload as { path?: string; focus?: boolean };
            if (detail?.path) {
              window.dispatchEvent(
                new CustomEvent("vibn://open-file", { detail }),
              );
              setExplorerOpen(true);
              try {
                localStorage.setItem("vibn:explorerOpen", "1");
              } catch {
                /* ignore */
              }
            }
          }
        }
      } catch {
        /* ignore */
      }
    };
    const id = setInterval(tick, 600);
    void tick();
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [auth.kind]);

  // -------- Auth bootstrap --------
  useEffect(() => {
    (async () => {
      const profile = await api.getUserProfile();
      if (profile.signed_in && profile.email) {
        setAuth({ kind: "ready", profile });
        return;
      }
      let remembered: string | null = null;
      try {
        if (profile.email) remembered = await api.getCredential(`password:${profile.email}`);
      } catch {
        /* ignore */
      }
      setAuth({ kind: "auth", profile, remembered });
    })();
  }, []);

  // -------- Models + transcripts + slash --------
  useEffect(() => {
    if (auth.kind !== "ready") return;
    (async () => {
      try {
        const m = await api.listModels();
        setModels(m);
      } catch {
        setModels([]);
      }
      try {
        setActiveModel(await api.activeModel());
      } catch {
        /* ignore */
      }
      try {
        setTranscripts(await api.listTranscripts(50));
      } catch {
        setTranscripts([]);
      }
      try {
        setSlashCommands(await api.listSlashCommands());
      } catch {
        setSlashCommands([]);
      }
    })();
  }, [auth.kind]);

  const refreshTranscripts = useCallback(async () => {
    try {
      setTranscripts(await api.listTranscripts(50));
    } catch {
      /* ignore */
    }
  }, []);

  // Listen for tray + titlebar events + install/download progress events
  useEffect(() => {
    if (auth.kind !== "ready") return;
    const unlisten: Array<() => void> = [];
    const onNewChat = () => {
      setView("chat");
      setActiveSession(null);
      setMessages([]);
    };
    const onOpenSettings = () => setView("settings");
    window.addEventListener("vibn:new-chat", onNewChat);
    window.addEventListener("vibn:open-settings", onOpenSettings);
    unlisten.push(() => window.removeEventListener("vibn:new-chat", onNewChat));
    unlisten.push(() => window.removeEventListener("vibn:open-settings", onOpenSettings));
    getCurrentWindow()
      .listen("vibn://new-chat", () => {
        setActiveSession(null);
        setMessages([]);
      })
      .then((u) => unlisten.push(u));
    getCurrentWindow()
      .listen<string>("vibn://install-progress", (evt) => {
        const line = String(evt.payload ?? "");
        if (!line) return;
        setMessages((m) => {
          // collapse the most recent tool/system progress line into a single
          // updating message so we don't spam the thread.
          const last = m[m.length - 1];
          if (last && last.role === "tool" && last.name === "install_progress") {
            return [
              ...m.slice(0, -1),
              { role: "tool", name: "install_progress", content: line },
            ];
          }
          return [...m, { role: "tool", name: "install_progress", content: line }];
        });
      })
      .then((u) => unlisten.push(u));
    return () => {
      unlisten.forEach((u) => u());
    };
  }, [auth.kind]);

  // -------- Chat actions --------
  const newChat = useCallback(() => {
    setView("chat");
    setActiveSession(null);
    setMessages([]);
  }, []);

  const openTranscript = useCallback(async (sessionId: string) => {
    try {
      const payload = await api.loadTranscript(sessionId);
      setActiveSession(payload.session_id);
      setMessages(payload.messages.filter((m) => m.role !== "system"));
    } catch (e) {
      console.error("openTranscript", e);
    }
  }, []);

  const sendUserMessage = useCallback(
    async (text: string) => {
      if (busy) return;
      const trimmed = text.trim();
      if (!trimmed) return;
      if (!activeModel) {
        alert("Pick a model first.");
        return;
      }
      const userMsg: ChatMessage = { role: "user", content: trimmed };
      const base = [...messages, userMsg];
      setMessages(base);
      setBusy(true);
      try {
        const out = await api.sendMessage(base, activeModel, activeSession ?? undefined);
        setMessages(out.messages.filter((m) => m.role !== "system"));
        setActiveSession(out.session_id);
        refreshTranscripts();
      } catch (e) {
        setMessages([
          ...base,
          { role: "assistant", content: `[error] ${String(e)}` },
        ]);
      } finally {
        setBusy(false);
      }
    },
    [busy, activeModel, messages, refreshTranscripts],
  );

  const runSlashCommand = useCallback(
    async (cmd: string, args: string) => {
      const annotate = (text: string) =>
        setMessages((m) => [
          ...m,
          { role: "tool", name: cmd, content: text },
        ]);
      annotate(`${cmd}${args ? ` ${args}` : ""}`);
      try {
        switch (cmd) {
          case "/new":
          case "/reset":
            newChat();
            return;
          case "/quit":
          case "/exit":
            window.close();
            return;
          case "/help": {
            const lines = slashCommands.map((c) => `${c.command}  —  ${c.description}`);
            annotate(lines.join("\n"));
            return;
          }
          case "/model":
            if (args) {
              setActiveModel(args.trim());
              await api.setActiveModel(args.trim());
              annotate(`Switched model → ${args.trim()}`);
            }
            return;
          case "/vision-model":
          case "/image-model":
          case "/video-model":
          case "/comfy-url": {
            const keyMap: Record<string, string> = {
              "/vision-model": "vision_model",
              "/image-model": "image_gen_model",
              "/video-model": "video_gen_model",
              "/comfy-url": "comfyui_url",
            };
            if (!args) {
              setView("settings");
              return;
            }
            await api.setConfigField(keyMap[cmd], args.trim());
            annotate(`${keyMap[cmd]} = ${args.trim()}`);
            return;
          }
          case "/install-comfy": {
            annotate("Installing managed ComfyUI… this can take a while.");
            const log = await api.installComfyui();
            for (const line of log) annotate(line);
            return;
          }
          case "/start-comfy":
            annotate(await api.startComfyui());
            return;
          case "/stop-comfy":
            annotate(await api.stopComfyui());
            return;
          case "/download-image-model": {
            if (!args) {
              annotate("Usage: /download-image-model comfyui:sdxl-base");
              return;
            }
            const log = await api.downloadImageModel(args.trim());
            for (const line of log) annotate(line);
            return;
          }
          case "/transcripts":
          case "/sessions":
            await refreshTranscripts();
            annotate(`Loaded ${transcripts.length} conversations.`);
            return;
          case "/mcp":
            annotate(await api.runSlash("/mcp"));
            return;
          case "/remember":
            if (!args) {
              annotate("Usage: /remember TEXT");
              return;
            }
            annotate(await api.runSlash("/remember", args));
            return;
          case "/memory":
            annotate(await api.runSlash("/memory"));
            return;
          default:
            try {
              annotate(await api.runSlash(cmd, args));
            } catch (e) {
              annotate(String(e));
            }
        }
      } catch (e) {
        annotate(`[error] ${String(e)}`);
      }
    },
    [slashCommands, newChat, refreshTranscripts, transcripts.length],
  );

  const onComposerSubmit = useCallback(
    (text: string) => {
      const t = text.trim();
      if (!t) return;
      if (t.startsWith("/")) {
        const space = t.indexOf(" ");
        const cmd = space === -1 ? t : t.slice(0, space);
        const args = space === -1 ? "" : t.slice(space + 1);
        return runSlashCommand(cmd, args);
      }
      return sendUserMessage(t);
    },
    [runSlashCommand, sendUserMessage],
  );

  // Memoize current chat preview for sidebar
  const currentChatPreview = useMemo(() => {
    if (activeSession || messages.length === 0) return undefined;
    return messages.find((m) => m.role === "user")?.content ?? undefined;
  }, [activeSession, messages]);

  // -------- Render --------
  if (auth.kind === "loading") {
    return (
      <div
        className="fixed inset-0 grid place-items-center text-white/40 text-xs"
        style={{ paddingTop: "var(--titlebar-h)" }}
      >
        …
      </div>
    );
  }
  if (auth.kind === "auth") {
    return (
      <AuthScreen
        initialProfile={auth.profile}
        rememberedPassword={auth.remembered}
        onAuthenticated={(p) => setAuth({ kind: "ready", profile: p })}
      />
    );
  }

  return (
    <div className="vibn-shell">
      <TitleBar view={view} onChangeView={setView} profile={auth.profile} />

      {view === "marketplace" && <MarketplaceView />}
      {view === "settings" && <SettingsView profile={auth.profile} models={models} />}
      {view === "chat" && (
        <div className="min-h-0 min-w-0 overflow-hidden flex">
          <ResizablePanelGroup direction="horizontal" className="h-full w-full">
            <ResizablePanel
              defaultSize={22}
              minSize={14}
              maxSize={36}
              className="flex flex-col min-h-0 min-w-0 bg-black/25 overflow-hidden"
            >
              <Sidebar
                collapsed={false}
                transcripts={transcripts}
                activeSession={activeSession}
                currentChatPreview={currentChatPreview}
                onOpenTranscript={openTranscript}
                onNewChat={newChat}
              />
            </ResizablePanel>

            <ResizableHandle />

            <ResizablePanel
              defaultSize={explorerOpen && activeProject ? 48 : 78}
              minSize={32}
              className="flex flex-col min-h-0 min-w-0 overflow-hidden"
            >
              <ChatHeader
                title={activeSession ? transcripts.find((t) => t.session_id === activeSession)?.title ?? "Conversation" : "New conversation"}
                subtitle={transcripts.find((t) => t.session_id === activeSession)?.project ?? ""}
                canDelete={!!activeSession || messages.length > 0}
                onDelete={async () => {
                  if (activeSession) {
                    try {
                      await api.deleteTranscript(activeSession);
                    } catch (e) {
                      console.error(e);
                    }
                  }
                  setActiveSession(null);
                  setMessages([]);
                  refreshTranscripts();
                }}
                onActiveProjectChange={onActiveProjectChange}
                explorerOpen={explorerOpen}
                canShowExplorer={!!activeProject}
                onToggleExplorer={toggleExplorer}
              />
              <div className="flex-1 min-h-0 overflow-y-auto vibn-scroll">
                {messages.length === 0 ? (
                  <EmptyState
                    activeModel={activeModel}
                    onAction={(cmd, args) => runSlashCommand(cmd, args ?? "")}
                    onPromptHint={(text) => onComposerSubmit(text)}
                  />
                ) : (
                  <Messages
                    messages={messages}
                    busy={busy}
                    onAction={(cmd, args) => runSlashCommand(cmd, args ?? "")}
                  />
                )}
              </div>
              <Composer
                slashCommands={slashCommands}
                busy={busy}
                onSubmit={onComposerSubmit}
                installedModels={models}
                activeModel={activeModel}
                onChangeModel={async (m) => {
                  setActiveModel(m);
                  await api.setActiveModel(m);
                }}
              />
            </ResizablePanel>

            {explorerOpen && activeProject && (
              <>
                <ResizableHandle />
                <ResizablePanel
                  defaultSize={32}
                  minSize={22}
                  maxSize={60}
                  className="flex flex-col min-h-0 min-w-0 overflow-hidden"
                >
                  <ExplorerPanel
                    projectPath={activeProject.path}
                    onClose={toggleExplorer}
                  />
                </ResizablePanel>
              </>
            )}
          </ResizablePanelGroup>
        </div>
      )}
    </div>
  );
}
