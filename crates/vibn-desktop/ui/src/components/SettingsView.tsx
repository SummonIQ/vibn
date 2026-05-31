import { useCallback, useEffect, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Button } from "./ui/button";
import { Select, type SelectItem } from "./ui/select";
import { api } from "../api";
import type {
  ConfigPayload,
  DesktopPermissions,
  McpServerEntry,
  MemoryEntry,
  ModelEntry,
  UserProfile,
} from "../types";
import { cn } from "../lib/utils";

interface Props {
  profile: UserProfile;
  models: ModelEntry[];
}

type Tab = "profile" | "models" | "comfy" | "mcp" | "memory" | "desktop";

const TABS: { id: Tab; label: string }[] = [
  { id: "profile", label: "Profile" },
  { id: "models", label: "Models" },
  { id: "comfy", label: "ComfyUI" },
  { id: "mcp", label: "MCP" },
  { id: "memory", label: "Memory" },
  { id: "desktop", label: "Desktop" },
];

export function SettingsView({ profile, models }: Props) {
  const [tab, setTab] = useState<Tab>("profile");
  const [config, setConfig] = useState<ConfigPayload | null>(null);
  const [memory, setMemory] = useState<MemoryEntry[]>([]);
  const [mcp, setMcp] = useState<McpServerEntry[]>([]);

  useEffect(() => {
    api.getConfig().then(setConfig).catch(() => {});
    api.listProjectMemory().then(setMemory).catch(() => setMemory([]));
    api.listMcpServers().then(setMcp).catch(() => setMcp([]));
  }, []);

  const modelItems: SelectItem<string>[] = models
    .map((m) => ({ value: m.name, label: m.name }))
    .concat(
      [
        "comfyui:flux1-schnell",
        "comfyui:sdxl-base",
        "comfyui:sd15",
        "comfyui:ltx-video",
        "qwen2.5vl:7b",
        "moondream:1.8b",
        "llama3.2-vision:11b",
      ]
        .filter((n) => !models.some((m) => m.name === n))
        .map((n) => ({ value: n, label: n })),
    );

  const setField = async (key: string, value: unknown) => {
    const c = await api.setConfigField(key, value);
    setConfig(c);
  };

  return (
    <div className="flex flex-col h-full min-h-0 bg-[#0c0c12]">
      <div className="px-6 pt-5 pb-3 border-b border-white/[0.05]">
        <h1 className="text-[18px] font-semibold tracking-tight">Settings</h1>
        <p className="text-[12px] text-white/45 mt-0.5">Configure models, integrations, memory, and permissions</p>
      </div>
      <div className="flex flex-1 min-h-0">
        <nav className="w-[180px] flex-shrink-0 border-r border-white/[0.05] py-3 px-2 flex flex-col gap-px">
          {TABS.map((t) => (
            <button
              key={t.id}
              onClick={() => setTab(t.id)}
              className={cn(
                "px-3 py-1.5 text-[12.5px] rounded-md transition-colors text-left",
                tab === t.id
                  ? "bg-white/[0.07] text-white"
                  : "text-white/55 hover:text-white/85 hover:bg-white/[0.035]",
              )}
            >
              {t.label}
            </button>
          ))}
        </nav>

        <div className="flex-1 min-h-0 overflow-y-auto vibn-scroll p-6">
          <AnimatePresence mode="wait">
            <motion.div
              key={tab}
              initial={{ opacity: 0, y: 4 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={{ duration: 0.12 }}
              className="max-w-[520px] flex flex-col gap-4"
            >
              {tab === "profile" && (
                <ProfilePane profile={profile} />
              )}
              {tab === "models" && (
                <>
                  <Field label="Default model">
                    <Select
                      value={config?.default_model ?? ""}
                      onChange={(v) => setField("default_model", v)}
                      items={modelItems}
                      placeholder="select…"
                    />
                  </Field>
                  <Field label="Vision model">
                    <Select
                      value={(config?.extra?.vision_model as string) ?? ""}
                      onChange={(v) => setField("vision_model", v)}
                      items={modelItems}
                      placeholder="qwen2.5vl:7b"
                    />
                  </Field>
                  <Field label="Image gen model">
                    <Select
                      value={(config?.extra?.image_gen_model as string) ?? ""}
                      onChange={(v) => setField("image_gen_model", v)}
                      items={modelItems}
                      placeholder="comfyui:sdxl-base"
                    />
                  </Field>
                  <Field label="Video gen model">
                    <Select
                      value={(config?.extra?.video_gen_model as string) ?? ""}
                      onChange={(v) => setField("video_gen_model", v)}
                      items={modelItems}
                      placeholder="comfyui:ltx-video"
                    />
                  </Field>
                </>
              )}
              {tab === "comfy" && (
                <ComfyPane
                  url={(config?.extra?.comfyui_url as string) ?? ""}
                  onChangeUrl={(v) => setField("comfyui_url", v)}
                />
              )}
              {tab === "mcp" && <McpPane mcp={mcp} refresh={() => api.listMcpServers().then(setMcp)} />}
              {tab === "memory" && (
                <MemoryPane
                  memory={memory}
                  refresh={() => api.listProjectMemory().then(setMemory)}
                />
              )}
              {tab === "desktop" && (
                <DesktopPane
                  enabled={(config?.extra?.enable_desktop_tools as boolean) ?? false}
                  onChange={(v) => setField("enable_desktop_tools", v)}
                />
              )}
            </motion.div>
          </AnimatePresence>
        </div>
      </div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-[10.5px] uppercase tracking-[0.06em] text-white/40">{label}</span>
      {children}
    </label>
  );
}

function ProfilePane({ profile }: { profile: UserProfile }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-3 p-2 rounded-lg bg-white/[0.025] border border-white/[0.05]">
        <div className="h-9 w-9 rounded-full grid place-items-center bg-gradient-to-br from-violet-400 to-purple-700 text-white font-bold text-[14px]">
          {(profile.display_name || profile.email).slice(0, 1).toUpperCase()}
        </div>
        <div className="min-w-0">
          <div className="text-[12.5px] font-semibold truncate">
            {profile.display_name || profile.email.split("@")[0] || "guest"}
          </div>
          <div className="text-[11px] text-white/45 truncate">{profile.email}</div>
        </div>
      </div>
      <div className="text-[11px] text-white/40">
        Auth endpoint: <span className="text-white/65">{profile.auth_endpoint || "(local profile)"}</span>
      </div>
      <div className="text-[11px] text-white/40">
        Status: <span className={profile.signed_in ? "text-emerald-400" : "text-amber-400"}>{profile.signed_in ? "signed in" : "local only"}</span>
      </div>
    </div>
  );
}

function ComfyPane({ url, onChangeUrl }: { url: string; onChangeUrl: (v: string) => void }) {
  const [local, setLocal] = useState(url);
  const [busy, setBusy] = useState(false);
  const [log, setLog] = useState<string[]>([]);
  useEffect(() => setLocal(url), [url]);
  return (
    <div className="flex flex-col gap-3">
      <Field label="ComfyUI URL">
        <input
          value={local}
          onChange={(e) => setLocal(e.target.value)}
          onBlur={() => onChangeUrl(local)}
          placeholder="http://127.0.0.1:8188"
          className="w-full bg-zinc-800/70 border border-white/10 rounded-md px-2.5 py-1.5 text-[12.5px] focus:outline-none focus:bg-zinc-800"
        />
      </Field>
      <div className="flex gap-1.5">
        <Button
          variant="primary"
          size="sm"
          loading={busy}
          onClick={async () => {
            setBusy(true);
            setLog(["Installing managed ComfyUI…"]);
            try {
              const out = await api.installComfyui();
              setLog((l) => [...l, ...out]);
            } catch (e) {
              setLog((l) => [...l, `Error: ${String(e)}`]);
            }
            setBusy(false);
          }}
        >
          Install
        </Button>
        <Button variant="secondary" size="sm" onClick={async () => setLog([await api.startComfyui()])}>
          Start
        </Button>
        <Button variant="ghost" size="sm" onClick={async () => setLog([await api.stopComfyui()])}>
          Stop
        </Button>
        <Button
          variant="secondary"
          size="sm"
          onClick={async () => {
            setBusy(true);
            setLog(["Downloading comfyui:sdxl-base…"]);
            try {
              const out = await api.downloadImageModel("comfyui:sdxl-base");
              setLog((l) => [...l, ...out]);
            } catch (e) {
              setLog((l) => [...l, `Error: ${String(e)}`]);
            }
            setBusy(false);
          }}
        >
          Get SDXL
        </Button>
      </div>
      {log.length > 0 && (
        <pre className="text-[10.5px] font-mono text-white/55 bg-black/30 rounded-md p-2 max-h-[120px] overflow-y-auto vibn-scroll whitespace-pre-wrap">
          {log.join("\n")}
        </pre>
      )}
    </div>
  );
}

function McpPane({ mcp, refresh }: { mcp: McpServerEntry[]; refresh: () => void }) {
  const [name, setName] = useState("");
  const [command, setCommand] = useState("");
  const [args, setArgs] = useState("");
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-1">
        {mcp.length === 0 ? (
          <div className="text-[11.5px] text-white/40 px-1 py-2">No MCP servers configured.</div>
        ) : (
          mcp.map((s) => (
            <div
              key={s.name}
              className="flex items-center justify-between gap-2 p-2 rounded-lg bg-white/[0.025] border border-white/[0.05]"
            >
              <div className="min-w-0">
                <div className="text-[12px] font-medium flex items-center gap-1.5">
                  <span className={cn("h-1.5 w-1.5 rounded-full", s.connected ? "bg-emerald-400" : "bg-white/20")} />
                  {s.name}
                </div>
                <div className="text-[10.5px] text-white/40 truncate">{s.command} {s.args.join(" ")}</div>
                {s.connected && <div className="text-[10px] text-white/30">{s.tool_count} tools</div>}
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={async () => {
                  await api.removeMcpServer(s.name);
                  refresh();
                }}
              >
                Remove
              </Button>
            </div>
          ))
        )}
      </div>
      <div className="border-t border-white/5 pt-3 flex flex-col gap-1.5">
        <div className="text-[10.5px] uppercase tracking-[0.06em] text-white/40">add server</div>
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="name (e.g. filesystem)"
          className="w-full bg-zinc-800/70 border border-white/10 rounded-md px-2.5 py-1.5 text-[12.5px] focus:outline-none focus:bg-zinc-800"
        />
        <input
          value={command}
          onChange={(e) => setCommand(e.target.value)}
          placeholder="command (e.g. npx)"
          className="w-full bg-zinc-800/70 border border-white/10 rounded-md px-2.5 py-1.5 text-[12.5px] focus:outline-none focus:bg-zinc-800"
        />
        <input
          value={args}
          onChange={(e) => setArgs(e.target.value)}
          placeholder="args (space-separated)"
          className="w-full bg-zinc-800/70 border border-white/10 rounded-md px-2.5 py-1.5 text-[12.5px] focus:outline-none focus:bg-zinc-800"
        />
        <Button
          variant="primary"
          size="sm"
          disabled={!name || !command}
          onClick={async () => {
            try {
              await api.addMcpServer(name, command, args.trim() ? args.trim().split(/\s+/) : []);
              setName("");
              setCommand("");
              setArgs("");
              refresh();
            } catch (e) {
              alert(String(e));
            }
          }}
        >
          Add MCP server
        </Button>
      </div>
    </div>
  );
}

function MemoryPane({ memory, refresh }: { memory: MemoryEntry[]; refresh: () => void }) {
  const [text, setText] = useState("");
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-1">
        {memory.length === 0 ? (
          <div className="text-[11.5px] text-white/40 px-1 py-2">No remembered facts for this project.</div>
        ) : (
          memory.map((m, i) => (
            <div
              key={i}
              className="flex items-start justify-between gap-2 p-2 rounded-lg bg-white/[0.025] border border-white/[0.05]"
            >
              <div className="min-w-0">
                <div className="text-[12px] font-medium">{m.heading}</div>
                <div className="text-[11px] text-white/55 whitespace-pre-wrap">{m.text}</div>
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={async () => {
                  await api.forgetProjectMemory(i + 1);
                  refresh();
                }}
              >
                ✕
              </Button>
            </div>
          ))
        )}
      </div>
      <div className="border-t border-white/5 pt-3 flex flex-col gap-1.5">
        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="remember a fact about this project…"
          rows={2}
          className="w-full bg-zinc-800/70 border border-white/10 rounded-md px-2.5 py-1.5 text-[12.5px] resize-none focus:outline-none focus:bg-zinc-800"
        />
        <Button
          variant="primary"
          size="sm"
          disabled={!text.trim()}
          onClick={async () => {
            await api.saveObservation(text.trim(), "project");
            setText("");
            refresh();
          }}
        >
          Save
        </Button>
      </div>
    </div>
  );
}

function DesktopPane({
  enabled,
  onChange,
}: {
  enabled: boolean;
  onChange: (value: boolean) => void;
}) {
  const [perms, setPerms] = useState<DesktopPermissions | null>(null);
  const refresh = useCallback(() => {
    api.checkDesktopPermissions().then(setPerms).catch(() => setPerms(null));
  }, []);
  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 3000);
    return () => clearInterval(id);
  }, [refresh]);

  return (
    <div className="flex flex-col gap-3">
      <label className="flex items-start gap-3 p-3 rounded-lg bg-white/[0.025] border border-white/[0.05] cursor-pointer">
        <input
          type="checkbox"
          className="mt-[3px] h-3.5 w-3.5 accent-violet-500"
          checked={enabled}
          onChange={(e) => onChange(e.target.checked)}
        />
        <div className="min-w-0">
          <div className="text-[12.5px] font-semibold">Allow Vibn to see and control other apps</div>
          <div className="mt-1 text-[11px] text-white/55 leading-snug">
            Enables tools the agent can use to list windows, focus apps, take screenshots, read selected
            text, send keystrokes, and run AppleScript. Every call still requires per-tool permission.
            macOS only at the moment.
          </div>
        </div>
      </label>

      {enabled && perms && !perms.supported && (
        <div className="rounded-lg border border-amber-500/30 bg-amber-500/[0.05] text-amber-200 px-3 py-2 text-[11.5px]">
          Desktop use is currently only supported on macOS. Tools will return an error on this platform.
        </div>
      )}

      {enabled && perms?.supported && (
        <div className="flex flex-col gap-2">
          <PermissionRow
            label="Accessibility"
            description="Required for window control, keyboard input, and reading selected text."
            granted={perms.accessibility}
            onOpen={() => api.openSystemSettingsPane("accessibility").catch(() => {})}
          />
          <PermissionRow
            label="Screen Recording"
            description="Required for screenshots of windows or regions you don't own."
            granted={perms.screen_recording}
            onOpen={() => api.openSystemSettingsPane("screen_recording").catch(() => {})}
          />
          <button
            type="button"
            onClick={refresh}
            className="self-start text-[11px] text-white/50 hover:text-white underline-offset-2 hover:underline"
          >
            Re-check permissions
          </button>
        </div>
      )}
    </div>
  );
}

function PermissionRow({
  label,
  description,
  granted,
  onOpen,
}: {
  label: string;
  description: string;
  granted: boolean;
  onOpen: () => void;
}) {
  return (
    <div className="flex items-start gap-3 p-2.5 rounded-md bg-white/[0.02] border border-white/[0.04]">
      <span
        className={
          "mt-[3px] h-2 w-2 rounded-full " +
          (granted ? "bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.6)]" : "bg-red-400")
        }
        aria-hidden
      />
      <div className="min-w-0 flex-1">
        <div className="text-[12px] font-semibold">{label}</div>
        <div className="text-[11px] text-white/55 leading-snug">{description}</div>
      </div>
      {!granted && (
        <button
          type="button"
          onClick={onOpen}
          className="text-[10.5px] text-violet-200 border border-violet-500/30 rounded-md px-2 py-1 hover:bg-violet-500/[0.08]"
        >
          Open System Settings
        </button>
      )}
    </div>
  );
}

