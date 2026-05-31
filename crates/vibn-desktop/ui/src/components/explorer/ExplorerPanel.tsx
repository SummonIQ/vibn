import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "../../api";
import type { FileContent } from "../../types";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "../ui/resizable";
import { FileTree } from "./FileTree";
import { VibnEditor } from "./VibnEditor";

interface Props {
  projectPath: string;
  onClose?: () => void;
}

interface Tab {
  path: string;
  name: string;
  language: string;
  /** Currently loaded content (may include unsaved edits). */
  content: string;
  /** What was last loaded/saved — used to compute dirty. */
  saved: string;
  truncated: boolean;
  readOnly: boolean;
  loading: boolean;
}

function basename(path: string): string {
  const trimmed = path.replace(/\/+$/, "");
  const idx = trimmed.lastIndexOf("/");
  return idx === -1 ? trimmed : trimmed.slice(idx + 1) || trimmed;
}

export function ExplorerPanel({ projectPath, onClose }: Props) {
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [activePath, setActivePath] = useState<string | null>(null);

  const activeTab = useMemo(
    () => tabs.find((t) => t.path === activePath) ?? null,
    [tabs, activePath],
  );

  const upsertTab = useCallback((tab: Tab) => {
    setTabs((prev) => {
      const i = prev.findIndex((t) => t.path === tab.path);
      if (i === -1) return [...prev, tab];
      const next = prev.slice();
      next[i] = tab;
      return next;
    });
  }, []);

  const openFile = useCallback(
    async (path: string) => {
      setActivePath(path);
      const existing = tabs.find((t) => t.path === path);
      if (existing) return;
      // Insert a loading placeholder.
      const placeholder: Tab = {
        path,
        name: basename(path),
        language: "plaintext",
        content: "",
        saved: "",
        truncated: false,
        readOnly: true,
        loading: true,
      };
      setTabs((prev) => [...prev, placeholder]);
      try {
        const fc: FileContent = await api.readProjectFile(path);
        upsertTab({
          path: fc.path,
          name: basename(fc.path),
          language: fc.language,
          content: fc.content,
          saved: fc.content,
          truncated: fc.truncated,
          readOnly: true,
          loading: false,
        });
      } catch (err) {
        upsertTab({
          path,
          name: basename(path),
          language: "plaintext",
          content: `// Failed to open: ${err}`,
          saved: `// Failed to open: ${err}`,
          truncated: false,
          readOnly: true,
          loading: false,
        });
      }
    },
    [tabs, upsertTab],
  );

  const closeTab = useCallback(
    (path: string) => {
      setTabs((prev) => {
        const next = prev.filter((t) => t.path !== path);
        if (activePath === path) {
          setActivePath(next.length > 0 ? next[next.length - 1].path : null);
        }
        return next;
      });
    },
    [activePath],
  );

  const handleChange = useCallback(
    (path: string, content: string) => {
      setTabs((prev) =>
        prev.map((t) => (t.path === path ? { ...t, content } : t)),
      );
    },
    [],
  );

  const handleSave = useCallback(
    async (path: string) => {
      const tab = tabs.find((t) => t.path === path);
      if (!tab || tab.readOnly || tab.loading) return;
      if (tab.content === tab.saved) return;
      try {
        await api.writeProjectFile(tab.path, tab.content);
        setTabs((prev) =>
          prev.map((t) => (t.path === path ? { ...t, saved: t.content } : t)),
        );
      } catch (err) {
        console.error("[vibn] save failed", err);
      }
    },
    [tabs],
  );

  const toggleEdit = useCallback((path: string) => {
    setTabs((prev) =>
      prev.map((t) => (t.path === path ? { ...t, readOnly: !t.readOnly } : t)),
    );
  }, []);

  // Listen for agent-driven open events.
  useEffect(() => {
    const handler = (ev: Event) => {
      const detail = (ev as CustomEvent<{ path?: string }>).detail;
      if (detail?.path) {
        openFile(detail.path);
      }
    };
    window.addEventListener("vibn://open-file", handler as EventListener);
    return () => {
      window.removeEventListener("vibn://open-file", handler as EventListener);
    };
  }, [openFile]);

  return (
    <ResizablePanelGroup direction="horizontal" className="h-full bg-zinc-950/40 border-l border-white/5">
      <ResizablePanel defaultSize={28} minSize={18} maxSize={45}>
        <div className="flex h-full flex-col">
          <div className="flex items-center justify-between gap-2 px-3 py-2 border-b border-white/5 text-[11px] uppercase tracking-wider text-white/45">
            <span>Files</span>
            {onClose && (
              <button
                type="button"
                onClick={onClose}
                aria-label="Close explorer"
                className="h-5 w-5 grid place-items-center rounded text-white/50 hover:text-white hover:bg-white/[0.08]"
              >
                <svg viewBox="0 0 24 24" width="11" height="11" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M6 6l12 12M18 6L6 18" />
                </svg>
              </button>
            )}
          </div>
          <div className="flex-1 min-h-0 overflow-auto">
            <FileTree projectPath={projectPath} onOpenFile={openFile} />
          </div>
        </div>
      </ResizablePanel>

      <ResizableHandle />

      <ResizablePanel defaultSize={72} minSize={40}>
        <div className="flex h-full flex-col">
          <div className="flex items-stretch h-9 bg-zinc-950/60 border-b border-white/5 overflow-x-auto">
            {tabs.length === 0 ? (
              <div className="px-3 grid place-items-center text-[11px] text-white/35">
                Pick a file from the tree to open it
              </div>
            ) : (
              tabs.map((t) => {
                const dirty = !t.readOnly && t.content !== t.saved;
                const active = t.path === activePath;
                return (
                  <div
                    key={t.path}
                    className={
                      "group flex items-center gap-1.5 px-3 border-r border-white/5 text-[12px] cursor-pointer transition-colors " +
                      (active
                        ? "bg-zinc-950 text-white"
                        : "bg-zinc-950/40 text-white/55 hover:text-white hover:bg-zinc-950/70")
                    }
                    onClick={() => setActivePath(t.path)}
                    title={t.path}
                  >
                    <span className="truncate max-w-[160px]">{t.name}</span>
                    {dirty && <span className="h-1.5 w-1.5 rounded-full bg-violet-300/85" aria-label="unsaved" />}
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        closeTab(t.path);
                      }}
                      aria-label="Close tab"
                      className="opacity-0 group-hover:opacity-100 text-white/55 hover:text-white"
                    >
                      <svg viewBox="0 0 24 24" width="11" height="11" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                        <path d="M6 6l12 12M18 6L6 18" />
                      </svg>
                    </button>
                  </div>
                );
              })
            )}
          </div>

          {activeTab && (
            <div className="flex items-center justify-between gap-2 px-3 h-7 bg-zinc-950/40 border-b border-white/5 text-[10.5px] text-white/45">
              <div className="flex items-center gap-2 min-w-0">
                <span className="truncate">{activeTab.path}</span>
                {activeTab.truncated && (
                  <span className="rounded bg-amber-500/15 text-amber-200/85 px-1 py-[1px] text-[10px]">
                    truncated
                  </span>
                )}
              </div>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => toggleEdit(activeTab.path)}
                  className={
                    "px-2 py-[2px] rounded text-[10.5px] border " +
                    (activeTab.readOnly
                      ? "border-white/10 text-white/65 hover:text-white hover:bg-white/[0.06]"
                      : "border-violet-500/40 text-violet-200 bg-violet-500/[0.08]")
                  }
                >
                  {activeTab.readOnly ? "Edit" : "Editing"}
                </button>
                <button
                  type="button"
                  disabled={activeTab.readOnly || activeTab.content === activeTab.saved}
                  onClick={() => handleSave(activeTab.path)}
                  className="px-2 py-[2px] rounded text-[10.5px] border border-violet-500/40 text-violet-200 hover:bg-violet-500/[0.10] disabled:opacity-35 disabled:cursor-not-allowed"
                  title="⌘S"
                >
                  Save
                </button>
              </div>
            </div>
          )}

          <div className="flex-1 min-h-0">
            {activeTab ? (
              activeTab.loading ? (
                <div className="p-4 text-[11px] text-white/40">Loading {activeTab.name}…</div>
              ) : (
                <VibnEditor
                  key={activeTab.path}
                  path={activeTab.path}
                  language={activeTab.language}
                  value={activeTab.content}
                  readOnly={activeTab.readOnly}
                  onChange={(v) => handleChange(activeTab.path, v)}
                  onSave={() => handleSave(activeTab.path)}
                />
              )
            ) : (
              <div className="grid place-items-center h-full text-[12px] text-white/35">
                No file open
              </div>
            )}
          </div>
        </div>
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}
