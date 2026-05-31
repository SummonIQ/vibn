import { useMemo, useState } from "react";
import { Modal } from "./ui/modal";
import { Button } from "./ui/button";
import { api } from "../api";
import type { RegistryModel } from "../types";
import { cn } from "../lib/utils";

type Category = "all" | "code" | "vision" | "image" | "video" | "chat";

interface Props {
  open: boolean;
  onClose: () => void;
  registry: RegistryModel[];
  installed: Set<string>;
  onInstalled: (key: string) => void;
}

const CATEGORIES: { id: Category; label: string }[] = [
  { id: "all", label: "All" },
  { id: "code", label: "Code" },
  { id: "vision", label: "Vision" },
  { id: "image", label: "Image gen" },
  { id: "video", label: "Video gen" },
  { id: "chat", label: "Chat" },
];

function fits(m: RegistryModel, cat: Category): boolean {
  if (cat === "all") return true;
  if (cat === "vision") return m.vision;
  if (cat === "image") return m.source === "comfyui" && !m.key.includes("ltx-video");
  if (cat === "video") return m.source === "comfyui" && m.key.includes("video");
  if (cat === "code")
    return m.use_cases.some((u) => /cod|debug|refactor|test/i.test(u));
  if (cat === "chat") return !m.vision && m.source === "ollama";
  return true;
}

export function ModelBrowser({ open, onClose, registry, installed, onInstalled }: Props) {
  const [cat, setCat] = useState<Category>("all");
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [log, setLog] = useState<Record<string, string>>({});

  const list = useMemo(
    () => registry.filter((m) => fits(m, cat)).sort((a, b) => a.size_gb - b.size_gb),
    [registry, cat],
  );

  const install = async (m: RegistryModel) => {
    setBusyKey(m.key);
    setLog((l) => ({ ...l, [m.key]: "Starting…" }));
    try {
      if (m.source === "ollama") {
        const out = await api.pullOllamaModel(m.key);
        setLog((l) => ({ ...l, [m.key]: out }));
        onInstalled(m.key);
      } else if (m.source === "comfyui") {
        const out = await api.downloadImageModel(m.key);
        setLog((l) => ({ ...l, [m.key]: out.join("\n") }));
        onInstalled(m.key);
      } else {
        setLog((l) => ({ ...l, [m.key]: "Source not yet supported in the GUI. Use /model in CLI." }));
      }
    } catch (e) {
      setLog((l) => ({ ...l, [m.key]: String(e) }));
    } finally {
      setBusyKey(null);
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Model library"
      className="!max-w-3xl"
      bodyClassName="p-0"
    >
      <div className="flex flex-col max-h-[70vh]">
        <div className="flex items-center gap-1 px-4 py-2 border-b border-white/5 flex-wrap">
          {CATEGORIES.map((c) => (
            <button
              key={c.id}
              onClick={() => setCat(c.id)}
              className={cn(
                "px-2.5 py-1 rounded-md text-[11.5px] transition-colors",
                cat === c.id ? "bg-white/[0.08] text-white" : "text-white/45 hover:text-white/75",
              )}
            >
              {c.label}
            </button>
          ))}
        </div>
        <div className="flex-1 overflow-y-auto vibn-scroll px-5 py-4 flex flex-col gap-2">
          {list.length === 0 && (
            <div className="text-center text-[12px] text-white/40 py-6">No models in this category.</div>
          )}
          {list.map((m) => {
            const isInstalled = installed.has(m.key);
            const isBusy = busyKey === m.key;
            return (
              <div
                key={m.key}
                className="rounded-lg bg-white/[0.025] border border-white/[0.06] p-3 flex gap-3 items-start"
              >
                <div className="h-9 w-9 rounded-md bg-gradient-to-br from-violet-500/25 to-purple-700/15 border border-violet-400/15 grid place-items-center text-violet-300 flex-shrink-0 text-[10.5px] font-mono">
                  {m.source === "comfyui" ? "C" : m.source === "gguf" ? "G" : "O"}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="text-[12.5px] font-mono font-semibold truncate">{m.key}</span>
                    <span className="text-[10.5px] text-white/35">{m.size_gb} GB</span>
                    {m.vision && (
                      <span className="px-1.5 py-px rounded-full text-[9.5px] bg-violet-400/15 text-violet-200/85 border border-violet-400/20">
                        vision
                      </span>
                    )}
                    {m.tool_support && (
                      <span className="px-1.5 py-px rounded-full text-[9.5px] bg-emerald-400/15 text-emerald-200/85 border border-emerald-400/20">
                        tools
                      </span>
                    )}
                  </div>
                  <div className="text-[11.5px] text-white/55 mt-0.5">{m.summary}</div>
                  <div className="text-[10.5px] text-white/30 mt-0.5 truncate">
                    {m.use_cases.join(" · ")} · ≥ {m.min_ram_gb} GB RAM
                  </div>
                  {log[m.key] && (
                    <pre className="mt-2 text-[10.5px] text-white/55 bg-black/30 rounded-md p-2 max-h-[80px] overflow-y-auto vibn-scroll whitespace-pre-wrap">
                      {log[m.key]}
                    </pre>
                  )}
                </div>
                <div className="flex-shrink-0">
                  {isInstalled ? (
                    <span className="text-[11px] text-emerald-300/80 font-mono">installed</span>
                  ) : (
                    <Button
                      variant="primary"
                      size="sm"
                      loading={isBusy}
                      onClick={() => install(m)}
                    >
                      {isBusy ? "…" : "Install"}
                    </Button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </Modal>
  );
}
