import { motion, AnimatePresence } from "framer-motion";
import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../api";
import { Logo } from "./Logo";
import type { ChatMessage } from "../types";
import { cn } from "../lib/utils";

const IMG_EXT = /\.(png|jpe?g|gif|webp|bmp|svg)$/i;
const BASE64_IMG_RE = /data:image\/[a-zA-Z0-9.+-]+;base64,[A-Za-z0-9+/=]+/g;
// Match absolute / home-relative / cwd-relative paths. Allow backticks and
// quotes as the preceding boundary so "saved at `/Users/.../foo.png`" matches.
const PATH_RE = /(?:^|[\s`'"])((?:\/|~\/|\.\/)[^\s'"`<>]+)/g;
const THINK_BLOCK_RE = /<think>[\s\S]*?<\/think>/g;

function extractImagePaths(content: string): string[] {
  if (!content) return [];
  const without64 = content.replace(BASE64_IMG_RE, "");
  const found: string[] = [];
  const seen = new Set<string>();
  for (const match of without64.matchAll(PATH_RE)) {
    let p = match[1];
    // Trim trailing punctuation the model often appends after a path.
    p = p.replace(/[.,;:!?]+$/, "");
    if (!IMG_EXT.test(p)) continue;
    if (seen.has(p)) continue;
    seen.add(p);
    found.push(p);
  }
  return found;
}

interface ToolCall {
  name: string;
  arguments: Record<string, unknown>;
}

interface Confirmation {
  prompt: string;
  yesLabel: string;
  noLabel: string;
  action: { cmd: string; args?: string };
}

function detectToolCallJson(content: string): ToolCall | null {
  const t = (content ?? "").trim();
  if (!t.startsWith("{")) return null;
  try {
    const parsed = JSON.parse(t);
    return normalizeToolCall(parsed);
  } catch {
    /* not JSON */
  }
  return null;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function normalizeArguments(value: unknown): Record<string, unknown> {
  if (typeof value === "string") {
    try {
      return normalizeArguments(JSON.parse(value));
    } catch {
      return {};
    }
  }
  return asRecord(value) ?? {};
}

function normalizeToolCall(value: unknown): ToolCall | null {
  const record = asRecord(value);
  if (!record) return null;
  const fn = asRecord(record.function);
  const name =
    typeof record.name === "string"
      ? record.name
      : typeof fn?.name === "string"
        ? fn.name
        : null;
  if (!name) return null;
  return {
    name,
    arguments: normalizeArguments(record.arguments ?? fn?.arguments),
  };
}

function toolCallsFromMessage(message: ChatMessage): ToolCall[] {
  if (!Array.isArray(message.tool_calls)) return [];
  return message.tool_calls
    .map(normalizeToolCall)
    .filter((call): call is ToolCall => Boolean(call));
}

function isRawToolCallDump(content: string): boolean {
  const t = content
    .trim()
    .replace(/^```(?:json)?\s*/i, "")
    .replace(/```\s*$/i, "")
    .trim();
  if (!t.startsWith("{")) return false;
  return /"name"\s*:\s*"[a-z0-9_:-]+"/i.test(t) && /"arguments"\s*:/i.test(t);
}

function toolResultIsError(content: string, confirm: Confirmation | null): boolean {
  const lc = (content ?? "").toLowerCase();
  return Boolean(confirm) ||
    lc.includes("[blocked:") ||
    lc.startsWith("error:") ||
    lc.includes("requires user confirmation") ||
    lc.includes("error") ||
    lc.includes("not installed") ||
    lc.includes("not present") ||
    lc.includes("not reachable");
}

/** Scan for any embedded JSON blocks shaped like a tool call and pull them
 *  out of the message body so we can render them as separate chips. */
function extractEmbeddedCalls(content: string): { text: string; calls: ToolCall[] } {
  const calls: ToolCall[] = [];
  let out = "";
  let i = 0;
  const src = content ?? "";
  while (i < src.length) {
    const start = src.indexOf("{", i);
    if (start === -1) {
      out += src.slice(i);
      break;
    }
    // Quick sniff: does this look like a tool-call object?
    const head = src.slice(start, start + 80).replace(/\s/g, "");
    if (!head.startsWith('{"name":') && !head.startsWith('{"function":')) {
      out += src.slice(i, start + 1);
      i = start + 1;
      continue;
    }
    // Find matching close brace, respecting strings + escapes.
    let depth = 0;
    let end = -1;
    let inStr = false;
    let esc = false;
    for (let j = start; j < src.length; j++) {
      const c = src[j];
      if (esc) {
        esc = false;
        continue;
      }
      if (c === "\\") {
        esc = true;
        continue;
      }
      if (c === '"') {
        inStr = !inStr;
        continue;
      }
      if (inStr) continue;
      if (c === "{") depth++;
      else if (c === "}") {
        depth--;
        if (depth === 0) {
          end = j;
          break;
        }
      }
    }
    if (end === -1) {
      out += src.slice(i);
      break;
    }
    const block = src.slice(start, end + 1);
    try {
      const parsed = JSON.parse(block);
      const call = normalizeToolCall(parsed);
      if (call) {
        // strip leading prose between i..start AND the JSON itself
        out += src.slice(i, start);
        calls.push(call);
        i = end + 1;
        continue;
      }
    } catch {
      /* not a tool call */
    }
    // Not a tool call — keep verbatim.
    out += src.slice(i, end + 1);
    i = end + 1;
  }
  return { text: out.replace(/\n{3,}/g, "\n\n").trim(), calls };
}

function summarizeArgs(args: Record<string, unknown>): string {
  const keys = Object.keys(args);
  if (keys.length === 0) return "";
  const k = keys[0];
  const v = args[k];
  let s = typeof v === "string" ? v : JSON.stringify(v);
  if (s.length > 50) s = s.slice(0, 47) + "…";
  const extra = keys.length > 1 ? ` +${keys.length - 1}` : "";
  return `${k}: ${s}${extra}`;
}

function detectConfirmation(content: string): Confirmation | null {
  const lc = content.toLowerCase();
  if (lc.includes("comfyui is not installed") || lc.includes("comfyui isn't installed")) {
    return {
      prompt: "ComfyUI isn't installed yet. Install it locally? (~2 GB, 5–15 min)",
      yesLabel: "Install ComfyUI",
      noLabel: "Not now",
      action: { cmd: "/install-comfy" },
    };
  }
  const m = content.match(/(?:download_image_model|comfyui:)([a-z0-9_:.-]+)/i);
  if (m && (lc.includes("not present") || lc.includes("checkpoint"))) {
    const key = m[1].startsWith("comfyui:") ? m[1] : `comfyui:${m[1]}`;
    const sizeMatch = content.match(/~\s*([\d.]+)\s*GB/);
    const size = sizeMatch ? `~${sizeMatch[1]} GB` : "a few GB";
    return {
      prompt: `Download checkpoint \`${key}\` (${size})?`,
      yesLabel: "Download",
      noLabel: "Cancel",
      action: { cmd: "/download-image-model", args: key },
    };
  }
  return null;
}

function stripThinking(content: string): { text: string; thoughts: string[] } {
  const thoughts: string[] = [];
  const text = content.replace(THINK_BLOCK_RE, (m) => {
    thoughts.push(m.replace(/^<think>|<\/think>$/g, "").trim());
    return "";
  }).trim();
  return { text, thoughts };
}

function InlineImage({ path }: { path: string }) {
  const [src, setSrc] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copyState, setCopyState] = useState<"idle" | "copying" | "ok" | "err">("idle");
  const [saveState, setSaveState] = useState<"idle" | "saving" | "ok" | "err">("idle");

  useEffect(() => {
    let cancelled = false;
    api
      .readImageAsDataUrl(path)
      .then((url) => { if (!cancelled) setSrc(url); })
      .catch((e) => { if (!cancelled) setError(String(e)); });
    return () => { cancelled = true; };
  }, [path]);

  async function onCopy() {
    setCopyState("copying");
    try {
      await api.copyImageToClipboard(path);
      setCopyState("ok");
      setTimeout(() => setCopyState("idle"), 1400);
    } catch {
      setCopyState("err");
      setTimeout(() => setCopyState("idle"), 1800);
    }
  }

  async function onSave() {
    setSaveState("saving");
    try {
      const dest = await api.saveImageAs(path);
      setSaveState(dest ? "ok" : "idle");
      if (dest) setTimeout(() => setSaveState("idle"), 1400);
    } catch {
      setSaveState("err");
      setTimeout(() => setSaveState("idle"), 1800);
    }
  }

  if (error) {
    return (
      <div className="mt-2 text-[11px] text-red-300/80 font-mono">
        couldn’t load image: {path}
      </div>
    );
  }

  return (
    <div className="mt-3 inline-block max-w-full">
      <div className="rounded-lg border border-white/10 bg-black/30 overflow-hidden">
        {src ? (
          <img
            src={src}
            loading="lazy"
            className="block max-w-full max-h-[480px]"
            alt=""
          />
        ) : (
          <div className="h-40 w-64 animate-pulse bg-white/[0.04]" />
        )}
      </div>
      <div className="mt-1.5 flex items-center gap-1.5 text-[11px] text-white/70 font-sans">
        <button
          type="button"
          onClick={onSave}
          disabled={saveState === "saving"}
          className="px-2 py-1 rounded-md border border-white/10 hover:border-white/25 hover:text-white bg-white/[0.03] hover:bg-white/[0.06] transition-colors"
        >
          {saveState === "saving" ? "Saving…" : saveState === "ok" ? "Saved" : saveState === "err" ? "Save failed" : "Save"}
        </button>
        <button
          type="button"
          onClick={onCopy}
          disabled={copyState === "copying"}
          className="px-2 py-1 rounded-md border border-white/10 hover:border-white/25 hover:text-white bg-white/[0.03] hover:bg-white/[0.06] transition-colors"
        >
          {copyState === "copying" ? "Copying…" : copyState === "ok" ? "Copied" : copyState === "err" ? "Copy failed" : "Copy"}
        </button>
        <span className="ml-1 truncate text-white/35 font-mono text-[10.5px] max-w-[280px]" title={path}>
          {path.replace(/^.*\//, "")}
        </span>
      </div>
    </div>
  );
}

function MessageBody({
  content,
  skipImages = false,
}: {
  content: string;
  skipImages?: boolean;
}) {
  const base64Ref = useRef<HTMLDivElement>(null);
  const paths = useMemo(
    () => (skipImages ? [] : extractImagePaths(content)),
    [content, skipImages],
  );

  useEffect(() => {
    // Inline base64 images get appended imperatively because the source
    // strings are huge and we don't want them in the React tree.
    const target = base64Ref.current;
    if (!target) return;
    target.innerHTML = "";
    const inline = content.match(BASE64_IMG_RE) || [];
    inline.forEach((src) => {
      const img = document.createElement("img");
      img.src = src;
      img.className = "block max-w-full max-h-[480px] mt-2 rounded-lg border border-white/10";
      target.appendChild(img);
    });
  }, [content]);

  const text = content.replace(BASE64_IMG_RE, "").trim();
  return (
    <div>
      <pre className="whitespace-pre-wrap break-words font-sans text-[13.5px] leading-relaxed m-0">
        {text}
      </pre>
      {paths.map((p) => (
        <InlineImage key={p} path={p} />
      ))}
      <div ref={base64Ref} />
    </div>
  );
}

// ---------- Bubble components ----------

interface ToolStep {
  call: ToolCall | null;
  result: ChatMessage | null; // tool message
  confirm: Confirmation | null;
  status: "running" | "ok" | "error";
}

function StepChip({
  step,
  onAction,
  isLatest,
  busy,
  repeats,
}: {
  step: ToolStep;
  onAction: (cmd: string, args?: string) => void;
  isLatest: boolean;
  busy: boolean;
  repeats: number;
}) {
  const name = step.call?.name ?? step.result?.name ?? "tool";
  const resultImages = useMemo(
    () => (step.result?.content ? extractImagePaths(step.result.content) : []),
    [step.result?.content],
  );
  // generate_image is meant to be seen — open it by default so the user
  // doesn't have to click into a tool chip to find the picture they asked
  // for. Same for any tool whose result included an image path.
  const autoOpen = step.status === "error" || resultImages.length > 0 || name === "generate_image";
  const [open, setOpen] = useState(autoOpen);
  const [actioned, setActioned] = useState<"yes" | "no" | null>(null);
  const running = isLatest && busy && !step.result;

  const argLine = step.call ? summarizeArgs(step.call.arguments) : "";
  const friendlyResult = formatToolResult(step);

  // confirmation cards stay expanded
  const expandable = !!step.result;

  return (
    <div className="self-start max-w-[760px] w-full">
      <button
        type="button"
        disabled={!expandable}
        onClick={() => setOpen((o) => !o)}
        className={cn(
          "inline-flex items-center gap-2 max-w-full px-2.5 py-1 rounded-full border text-[11.5px] font-mono transition-colors",
          step.status === "error"
            ? "bg-red-500/[0.08] border-red-500/20 text-red-200/85"
            : "bg-violet-500/[0.07] border-violet-400/15 text-violet-200/85 hover:border-violet-400/30",
          !expandable && "cursor-default",
        )}
      >
        {running ? (
          <motion.span
            className="h-2.5 w-2.5 rounded-full border-2 border-current border-r-transparent flex-shrink-0"
            animate={{ rotate: 360 }}
            transition={{ duration: 0.9, repeat: Infinity, ease: "linear" }}
          />
        ) : step.status === "ok" ? (
          <svg viewBox="0 0 12 12" width="10" height="10" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="flex-shrink-0 opacity-60">
            <path d="M2 6l3 3 5-6" />
          </svg>
        ) : step.status === "error" ? (
          <svg viewBox="0 0 12 12" width="10" height="10" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" className="flex-shrink-0 opacity-60">
            <path d="M3 3l6 6M9 3l-6 6" />
          </svg>
        ) : (
          <svg viewBox="0 0 16 16" width="11" height="11" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" className="flex-shrink-0 opacity-60">
            <path d="M5 4l-3 4 3 4M11 4l3 4-3 4" />
          </svg>
        )}
        <span className="font-semibold flex-shrink-0">{name}</span>
        {argLine && <span className="text-violet-200/55 truncate">{argLine}</span>}
        {repeats > 1 && (
          <span className="flex-shrink-0 px-1.5 py-px rounded-full bg-white/[0.08] text-white/55 text-[9.5px] tabular-nums">
            ×{repeats}
          </span>
        )}
        {expandable && (
          <motion.svg
            viewBox="0 0 12 12"
            width="9"
            height="9"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.8"
            strokeLinecap="round"
            strokeLinejoin="round"
            className="flex-shrink-0 ml-0.5 opacity-50"
            animate={{ rotate: open ? 180 : 0 }}
            transition={{ duration: 0.18 }}
          >
            <path d="M3 4.5l3 3 3-3" />
          </motion.svg>
        )}
      </button>

      <AnimatePresence>
        {step.confirm && actioned === null && (
          <motion.div
            initial={{ opacity: 0, y: 4, height: 0 }}
            animate={{ opacity: 1, y: 0, height: "auto" }}
            exit={{ opacity: 0, y: 4, height: 0 }}
            transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
            className="mt-2 max-w-[640px] rounded-xl border bg-zinc-900/70 border-violet-400/20 px-3.5 py-3 flex flex-col gap-2.5 shadow-[0_0_22px_-8px_rgba(167,139,250,0.35)] overflow-hidden"
          >
            <div className="text-[13px] text-white/85 leading-snug">{step.confirm.prompt}</div>
            <div className="flex items-center gap-2">
              <button
                onClick={() => {
                  setActioned("yes");
                  onAction(step.confirm!.action.cmd, step.confirm!.action.args);
                }}
                className="h-7 px-3 rounded-md text-[12px] font-semibold bg-gradient-to-br from-violet-500/45 to-purple-900/30 border border-violet-400/40 text-white hover:from-violet-500/60 transition-all"
              >
                {step.confirm.yesLabel}
              </button>
              <button
                onClick={() => setActioned("no")}
                className="h-7 px-3 rounded-md text-[12px] text-white/55 hover:text-white hover:bg-white/[0.05] transition-colors"
              >
                {step.confirm.noLabel}
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <AnimatePresence initial={false}>
        {open && expandable && step.result && (
          <motion.div
            key="expand"
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
            className="overflow-hidden mt-2"
          >
            <div
              className={cn(
                "rounded-xl px-3 py-2 font-mono text-[11.5px] max-w-[760px]",
                step.status === "error"
                  ? "bg-red-500/[0.06] border border-red-400/15 text-red-200/85"
                  : "bg-emerald-500/[0.06] border border-emerald-400/15 text-emerald-200/85",
              )}
            >
              {resultImages.length > 0 && (
                <div className="flex flex-wrap gap-3 -mt-1 mb-2">
                  {resultImages.map((p) => (
                    <InlineImage key={p} path={p} />
                  ))}
                </div>
              )}
              {friendlyResult ? (
                <div className="font-sans leading-relaxed">
                  <div className="text-[13px] font-semibold text-white/90">
                    {friendlyResult.title}
                  </div>
                  <div className="mt-1 text-[12px] text-white/70">
                    {friendlyResult.body}
                  </div>
                  {friendlyResult.detail && (
                    <div className="mt-2 font-mono text-[11px] text-white/45 break-words">
                      {friendlyResult.detail}
                    </div>
                  )}
                </div>
              ) : (
                <MessageBody
                  content={step.result.content ?? ""}
                  skipImages={resultImages.length > 0}
                />
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function ThoughtChip({ thought, isLatest }: { thought: string; isLatest: boolean }) {
  const [open, setOpen] = useState(false);
  const preview = thought.replace(/\s+/g, " ").slice(0, 60) + (thought.length > 60 ? "…" : "");
  return (
    <div className="self-start max-w-[760px] w-full">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="inline-flex items-center gap-2 max-w-full px-2.5 py-1 rounded-full border bg-white/[0.025] border-white/[0.07] text-[11.5px] text-white/45 hover:border-white/15 transition-colors italic"
      >
        <svg viewBox="0 0 16 16" width="11" height="11" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" className="opacity-60 flex-shrink-0">
          <path d="M5 11a4 4 0 1 1 6 0v2H5v-2zM7 14h2" />
        </svg>
        <span className="flex-shrink-0">{isLatest ? "thinking" : "thought"}</span>
        <span className="text-white/30 truncate">· {preview}</span>
        <motion.svg
          viewBox="0 0 12 12"
          width="9"
          height="9"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="flex-shrink-0 ml-0.5 opacity-50"
          animate={{ rotate: open ? 180 : 0 }}
          transition={{ duration: 0.18 }}
        >
          <path d="M3 4.5l3 3 3-3" />
        </motion.svg>
      </button>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            transition={{ duration: 0.2 }}
            className="overflow-hidden mt-2"
          >
            <div className="rounded-xl px-3 py-2 text-[12px] text-white/55 leading-relaxed bg-white/[0.02] border border-white/[0.05] max-w-[760px] whitespace-pre-wrap">
              {thought}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

export interface MessagesProps {
  messages: ChatMessage[];
  busy: boolean;
  onAction: (cmd: string, args?: string) => void;
}

type Bubble =
  | { kind: "user"; m: ChatMessage }
  | { kind: "assistant"; m: ChatMessage }
  | { kind: "thought"; thought: string }
  | { kind: "step"; step: ToolStep; repeats: number }
  | { kind: "system"; m: ChatMessage };

function stepKey(s: ToolStep): string {
  const name = s.call?.name ?? s.result?.name ?? "tool";
  const args = s.call ? JSON.stringify(s.call.arguments) : "";
  const rc = (s.result?.content ?? "").slice(0, 200);
  return `${name}|${args}|${rc}`;
}

function formatToolResult(step: ToolStep): { title: string; body: string; detail?: string } | null {
  const content = (step.result?.content ?? "").trim();
  if (!content) return null;
  const modelKind = content.match(/^\[blocked:\s*model '([^']+)' is kind '([^']+)', not '([^']+)'\]$/i);
  if (modelKind) {
    const [, model, kind, expected] = modelKind;
    const kindArticle = /^[aeiou]/i.test(kind) ? "an" : "a";
    const expectedArticle = /^[aeiou]/i.test(expected) ? "an" : "a";
    return {
      title: expected === "video" ? "Video generation did not run" : "Tool did not run",
      body: `The selected model ${model} is ${kindArticle} ${kind} model, but this tool needs ${expectedArticle} ${expected} model.`,
      detail: expected === "video"
        ? "Set a video model in Settings, for example comfyui:ltx-video, then try again."
        : undefined,
    };
  }
  const blocked = content.match(/^\[blocked:\s*(.+)\]$/is);
  if (blocked) {
    return {
      title: "Tool blocked",
      body: blocked[1],
    };
  }
  return null;
}

export function Messages({ messages, busy, onAction }: MessagesProps) {
  const endRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [messages.length]);

  const bubbles = useMemo<Bubble[]>(() => {
    const out: Bubble[] = [];
    for (let i = 0; i < messages.length; i++) {
      const m = messages[i];
      if (m.role === "user") {
        out.push({ kind: "user", m });
      } else if (m.role === "assistant") {
        const { text: stripped, thoughts } = stripThinking(m.content ?? "");
        for (const t of thoughts) out.push({ kind: "thought", thought: t });
        // First, pull out any JSON tool-call blocks embedded in the text.
        const { text: prose, calls } = extractEmbeddedCalls(stripped);
        const nativeCalls = toolCallsFromMessage(m);
        const toolCalls = nativeCalls.length > 0 ? nativeCalls : calls;
        // Render the prose (if any).
        if (prose.length > 0 && !isRawToolCallDump(prose)) {
          out.push({ kind: "assistant", m: { ...m, content: prose } });
        }
        // Render each extracted call as its own step (consume the matching
        // tool-result messages in order).
        for (const tc of toolCalls) {
          const next = messages[i + 1];
          const result = next && next.role === "tool" ? next : null;
          if (result) i++;
          const confirm = result ? detectConfirmation(result.content ?? "") : null;
          const isError = result ? toolResultIsError(result.content ?? "", confirm) : false;
          const step: ToolStep = {
            call: tc,
            result,
            confirm,
            status: result ? (confirm ? "error" : isError ? "error" : "ok") : "running",
          };
          const prev = out[out.length - 1];
          if (prev && prev.kind === "step" && stepKey(prev.step) === stepKey(step)) {
            prev.repeats += 1;
          } else {
            out.push({ kind: "step", step, repeats: 1 });
          }
        }
      } else if (m.role === "tool") {
        // orphan tool (no preceding tool-call assistant message)
        const confirm = detectConfirmation(m.content ?? "");
        const isError = toolResultIsError(m.content ?? "", confirm);
        const step: ToolStep = {
          call: null,
          result: m,
          confirm,
          status: isError ? "error" : "ok",
        };
        const prev = out[out.length - 1];
        if (prev && prev.kind === "step" && stepKey(prev.step) === stepKey(step)) {
          prev.repeats += 1;
        } else {
          out.push({ kind: "step", step, repeats: 1 });
        }
      } else if (m.role === "system") {
        out.push({ kind: "system", m });
      }
    }
    return out;
  }, [messages]);

  // Find indices of the LAST step + LAST thought to mark as "current"
  const lastStepIdx = useMemo(
    () => {
      for (let i = bubbles.length - 1; i >= 0; i--) {
        if (bubbles[i].kind === "step") return i;
      }
      return -1;
    },
    [bubbles],
  );
  const lastThoughtIdx = useMemo(
    () => {
      for (let i = bubbles.length - 1; i >= 0; i--) {
        if (bubbles[i].kind === "thought") return i;
      }
      return -1;
    },
    [bubbles],
  );

  return (
    <div className="max-w-4xl mx-auto px-6 py-6 flex flex-col gap-3">
      <AnimatePresence initial={false}>
        {bubbles.map((b, i) => {
          if (b.kind === "user") {
            return (
              <motion.div
                key={i}
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
                className="flex justify-end"
              >
                <div className="max-w-[760px] px-3.5 py-2.5 rounded-2xl border bg-gradient-to-br from-violet-500/18 to-purple-500/12 border-violet-400/25 text-white text-[13.5px]">
                  <MessageBody content={b.m.content ?? ""} />
                </div>
              </motion.div>
            );
          }
          if (b.kind === "assistant") {
            return (
              <motion.div
                key={i}
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
                className="flex items-start gap-2.5"
              >
                <div className="flex-shrink-0 mt-0.5">
                  <Logo size={22} />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="text-[10px] tracking-[0.08em] uppercase text-white/40 mb-1">Vibn</div>
                  <div className="max-w-[720px] px-3.5 py-2.5 rounded-2xl border bg-zinc-900/70 border-white/[0.07] text-white/90 text-[13.5px]">
                    <MessageBody content={b.m.content ?? ""} />
                  </div>
                </div>
              </motion.div>
            );
          }
          if (b.kind === "thought") {
            return (
              <ThoughtChip
                key={i}
                thought={b.thought}
                isLatest={i === lastThoughtIdx && busy}
              />
            );
          }
          if (b.kind === "step") {
            return (
              <StepChip
                key={i}
                step={b.step}
                repeats={b.repeats}
                onAction={onAction}
                isLatest={i === lastStepIdx}
                busy={busy}
              />
            );
          }
          if (b.kind === "system") {
            return (
              <div
                key={i}
                className="self-center px-3 py-1 rounded-full bg-white/[0.04] border border-white/[0.06] text-[11px] text-white/45 font-mono"
              >
                {b.m.content}
              </div>
            );
          }
          return null;
        })}
      </AnimatePresence>
      {busy && <StatusBubble bubbles={bubbles} />}
      <div ref={endRef} />
    </div>
  );
}

function StatusBubble({ bubbles }: { bubbles: Bubble[] }) {
  const last = bubbles[bubbles.length - 1];
  let label = "Thinking";
  if (last) {
    if (last.kind === "step" && last.step.status === "running") {
      const name = last.step.call?.name ?? last.step.result?.name ?? "tool";
      label = `Running ${name}`;
    } else if (last.kind === "thought") {
      label = "Thinking";
    } else if (last.kind === "step") {
      label = "Reviewing result";
    } else if (last.kind === "user") {
      label = "Thinking";
    }
  }
  return (
    <motion.div
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.18 }}
      className="flex items-start gap-2.5"
      aria-live="polite"
    >
      <div className="flex-shrink-0 mt-0.5">
        <Logo size={22} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-[10px] tracking-[0.08em] uppercase text-white/40 mb-1">Vibn</div>
        <div className="inline-flex items-center gap-2 px-3 py-2 rounded-2xl border bg-zinc-900/70 border-white/[0.07] text-[13px] text-white/75">
          <span className="vibn-status-dots" aria-hidden>
            <span /><span /><span />
          </span>
          <span>{label}…</span>
        </div>
      </div>
    </motion.div>
  );
}
