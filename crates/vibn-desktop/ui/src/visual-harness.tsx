import { createRoot } from "react-dom/client";
import { useState } from "react";
import { EmptyState } from "./components/EmptyState";
import { Messages } from "./components/Messages";
import type { ChatMessage } from "./types";
import "./tailwind.css";
import "./style.css";

const sampleMessages: ChatMessage[] = [
  { role: "user", content: "Create a cool video" },
  {
    role: "assistant",
    content: "",
    tool_calls: [
      {
        function: {
          name: "generate_video",
          arguments: {
            prompt: "A stunning landscape with a majestic mountain range during sunset",
            frames: 120,
            height: 1080,
            model: "comfyui:sdxl-base",
          },
        },
      },
    ],
  },
  {
    role: "tool",
    name: "generate_video",
    content: "[blocked: model 'comfyui:sdxl-base' is kind 'image', not 'video']",
  },
  {
    role: "assistant",
    content: `{
  "name": "generate_video",
  "arguments": {
    "prompt": "A stunning landscape with a majestic mountain range during sunset",
    "model": "comfyui:sdxl-video", // invalid raw dump that should not render
    "frames": 120
  }
}`,
  },
];

function Harness() {
  const [view, setView] = useState<"messages" | "empty">("messages");
  return (
    <div className="min-h-screen bg-[#08090d] text-white">
      <div className="fixed top-4 left-4 z-20 flex gap-2">
        <button
          className="rounded-md border border-white/10 bg-white/[0.06] px-3 py-1.5 text-xs"
          onClick={() => setView("messages")}
        >
          Messages
        </button>
        <button
          className="rounded-md border border-white/10 bg-white/[0.06] px-3 py-1.5 text-xs"
          onClick={() => setView("empty")}
        >
          Empty
        </button>
      </div>
      {view === "messages" ? (
        <div className="pt-20">
          <Messages messages={sampleMessages} busy={false} onAction={() => undefined} />
        </div>
      ) : (
        <div className="h-screen pt-10">
          <EmptyState
            activeModel="llama3.2"
            onAction={() => undefined}
            onPromptHint={() => undefined}
          />
        </div>
      )}
    </div>
  );
}

const root = document.getElementById("root");
if (!root) throw new Error("missing root");
createRoot(root).render(<Harness />);
