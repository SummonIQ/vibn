import { motion, AnimatePresence } from "framer-motion";
import { useEffect, useMemo, useRef } from "react";
import { IconButton } from "./ui/icon-button";
import { formatRelativeTime } from "../chat";
import type { TranscriptSummary } from "../types";
import { cn } from "../lib/utils";

type Bucket = "today" | "yesterday" | "week" | "month" | "older";
const BUCKET_LABEL: Record<Bucket, string> = {
  today: "Today",
  yesterday: "Yesterday",
  week: "This Week",
  month: "This Month",
  older: "Older",
};
const BUCKET_ORDER: Bucket[] = ["today", "yesterday", "week", "month", "older"];

function bucketFor(timestamp: string): Bucket {
  const t = Date.parse(timestamp);
  if (Number.isNaN(t)) return "older";
  const diff = Date.now() - t;
  const day = 86400_000;
  if (diff < day) return "today";
  if (diff < day * 2) return "yesterday";
  if (diff < day * 7) return "week";
  if (diff < day * 30) return "month";
  return "older";
}

function groupByBucket(items: TranscriptSummary[]): { bucket: Bucket; items: TranscriptSummary[] }[] {
  const map = new Map<Bucket, TranscriptSummary[]>();
  for (const it of items) {
    const b = bucketFor(it.timestamp);
    const arr = map.get(b) ?? [];
    arr.push(it);
    map.set(b, arr);
  }
  const out: { bucket: Bucket; items: TranscriptSummary[] }[] = [];
  for (const b of BUCKET_ORDER) {
    const arr = map.get(b);
    if (arr && arr.length) out.push({ bucket: b, items: arr });
  }
  return out;
}

interface Props {
  collapsed: boolean;
  transcripts: TranscriptSummary[];
  activeSession: string | null;
  /** First user message of the in-progress conversation (no saved session id yet). */
  currentChatPreview?: string;
  onOpenTranscript: (id: string) => void;
  onNewChat: () => void;
}

export function Sidebar({
  collapsed,
  transcripts,
  activeSession,
  currentChatPreview,
  onOpenTranscript,
  onNewChat,
}: Props) {
  const groups = useMemo(() => groupByBucket(transcripts), [transcripts]);
  const showCurrent = !activeSession && !!currentChatPreview;
  const navRef = useRef<HTMLElement>(null);
  useEffect(() => {
    if (showCurrent && navRef.current) {
      navRef.current.scrollTop = 0;
    }
  }, [showCurrent, currentChatPreview]);
  return (
    <aside className="flex flex-col h-full min-w-0 overflow-hidden">
      <div className="flex items-center justify-between gap-2 px-3 h-11 border-b border-white/5">
        {!collapsed && (
          <span className="text-[10.5px] uppercase tracking-[0.1em] text-white/30">Conversations</span>
        )}
        <div className={cn("flex items-center gap-1", collapsed && "w-full justify-center")}>
          <IconButton
            aria-label="New chat"
            variant="ghost"
            size="sm"
            onClick={onNewChat}
            className="!text-white/55 hover:!text-white"
          >
            <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
              <path d="M8 3v10M3 8h10" />
            </svg>
          </IconButton>
        </div>
      </div>

      <nav ref={navRef} className="flex-1 min-h-0 overflow-y-auto vibn-scroll px-1.5 py-1.5">
        {collapsed ? (
          <div className="flex flex-col items-center gap-1.5 py-1">
            {transcripts.slice(0, 14).map((t) => (
              <button
                key={t.session_id}
                onClick={() => onOpenTranscript(t.session_id)}
                title={t.title}
                className={cn(
                  "h-6 w-6 grid place-items-center rounded-md text-[10px] font-medium",
                  "transition-colors",
                  activeSession === t.session_id
                    ? "bg-violet-500/15 text-white"
                    : "text-white/40 hover:text-white/80 hover:bg-white/[0.05]",
                )}
              >
                {(t.title || "·").slice(0, 1).toUpperCase()}
              </button>
            ))}
          </div>
        ) : transcripts.length === 0 && !showCurrent ? (
          <div className="text-center text-[11.5px] text-white/35 py-6">No previous chats</div>
        ) : (
          <div className="flex flex-col gap-5">
            {showCurrent && (
              <motion.section
                layout
                initial={{ opacity: 0, x: -8, scale: 0.96 }}
                animate={{ opacity: 1, x: 0, scale: 1 }}
                transition={{ duration: 0.32, ease: [0.22, 1, 0.36, 1] }}
                className="flex flex-col"
              >
                <div className="px-2 pt-1 pb-2 text-[9.5px] uppercase tracking-[0.12em] text-violet-300/80 font-semibold flex items-center gap-1.5">
                  <span className="h-1 w-1 rounded-full bg-violet-400 animate-pulse" />
                  Current
                </div>
                <div
                  className="relative w-full text-left rounded-lg px-2.5 py-2 mb-px flex flex-col gap-1 text-white overflow-hidden"
                  style={{
                    background:
                      "linear-gradient(135deg, rgba(167,139,250,0.18), rgba(124,58,237,0.08))",
                    border: "1px solid rgba(167,139,250,0.32)",
                    boxShadow:
                      "0 0 16px -4px rgba(167,139,250,0.35), inset 0 1px 0 rgba(255,255,255,0.06)",
                  }}
                >
                  <motion.div
                    className="absolute inset-0 pointer-events-none"
                    style={{
                      background:
                        "linear-gradient(120deg, transparent 40%, rgba(255,255,255,0.06) 50%, transparent 60%)",
                    }}
                    animate={{ backgroundPosition: ["-200% 0%", "200% 0%"] }}
                    transition={{ duration: 3, repeat: Infinity, ease: "linear" }}
                  />
                  <div className="relative flex items-baseline justify-between gap-2">
                    <div className="text-[12.5px] font-semibold truncate flex-1 leading-tight">
                      {currentChatPreview!.split("\n")[0].slice(0, 60) || "New conversation"}
                    </div>
                    <div className="text-[9.5px] text-violet-200/60 flex-shrink-0 tabular-nums">
                      now
                    </div>
                  </div>
                  <div className="relative text-[10.5px] text-white/55 truncate leading-tight">
                    {currentChatPreview!.slice(0, 120)}
                  </div>
                </div>
              </motion.section>
            )}
            {groups.map(({ bucket, items }) => (
              <section key={bucket} className="flex flex-col">
                <div className="px-2 pt-1 pb-2 text-[9.5px] uppercase tracking-[0.12em] text-white/30 font-semibold">
                  {BUCKET_LABEL[bucket]}
                </div>
                <AnimatePresence initial={false}>
                  {items.map((t) => {
                    const isActive = activeSession === t.session_id;
                    return (
                      <motion.button
                        key={t.session_id}
                        layout
                        onClick={() => onOpenTranscript(t.session_id)}
                        whileHover={{ x: 1 }}
                        transition={{ duration: 0.12 }}
                        className={cn(
                          "group w-full text-left rounded-md px-2 py-1.5 mb-px",
                          "flex flex-col gap-0.5 transition-colors duration-150",
                          isActive
                            ? "bg-violet-500/[0.10] text-white"
                            : "text-white/70 hover:bg-white/[0.035] hover:text-white",
                        )}
                      >
                        <div className="flex items-baseline justify-between gap-2">
                          <div className="text-[12px] font-medium truncate flex-1 leading-tight">
                            {t.title || "Untitled"}
                          </div>
                          <div className="text-[9.5px] text-white/30 flex-shrink-0 tabular-nums">
                            {formatRelativeTime(t.timestamp)}
                          </div>
                        </div>
                        <div className="text-[10.5px] text-white/40 truncate leading-tight">
                          {t.preview || "(no messages)"}
                        </div>
                      </motion.button>
                    );
                  })}
                </AnimatePresence>
              </section>
            ))}
          </div>
        )}
      </nav>
    </aside>
  );
}
