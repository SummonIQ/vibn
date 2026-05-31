import { api } from "./api";
import type { ChatMessage } from "./types";

const IMG_EXTENSIONS = /\.(png|jpe?g|gif|webp|bmp|svg)$/i;
const BASE64_IMG_RE = /data:image\/[a-zA-Z0-9.+-]+;base64,[A-Za-z0-9+/=]+/g;
const PATH_RE = /(?:^|\s)((?:\/|~\/|\.\/)[^\s'"`<>]+)/g;

export interface QuickAction {
  label: string;
  description: string;
  command?: string;
  onActivate?: () => void;
}

export function createMessageList(container: HTMLElement) {
  const clear = () => {
    container.innerHTML = "";
  };

  const appendMessage = (m: ChatMessage) => {
    const wrap = document.createElement("div");
    wrap.className = `msg msg-${m.role} msg-fade`;
    const role = document.createElement("span");
    role.className = "role";
    role.textContent = m.name ? `${m.role} · ${m.name}` : m.role;
    wrap.appendChild(role);
    const body = document.createElement("div");
    body.className = "body";
    wrap.appendChild(body);
    renderContent(body, m.content ?? "");
    container.appendChild(wrap);
    requestAnimationFrame(() => {
      container.scrollTop = container.scrollHeight;
    });
    return { el: wrap, contentEl: body };
  };

  const setMessages = (messages: ChatMessage[]) => {
    clear();
    for (const m of messages) appendMessage(m);
  };

  const renderLaunch = (actions: QuickAction[], activeModel: string) => {
    clear();
    const launch = document.createElement("div");
    launch.className = "launch";
    launch.innerHTML = `
      <div class="launch-hero">
        <div class="logo-dot"></div>
        <h1>Vibn</h1>
        <p class="tagline">Local AI coding agent. Reads files, edits with diff review, runs commands, sees images, and remembers what matters.</p>
        <p class="active-model">${activeModel ? `using <strong>${escapeHtml(activeModel)}</strong>` : "no model selected"}</p>
      </div>
      <div class="capabilities">
        <div class="cap"><span class="cap-icon">⌨️</span><div><div class="cap-title">Code</div><div class="cap-desc">Read, edit, refactor, debug, run tests, explain repos.</div></div></div>
        <div class="cap"><span class="cap-icon">🖼️</span><div><div class="cap-title">Vision</div><div class="cap-desc">Describe images, read screenshots, OCR, sample video frames.</div></div></div>
        <div class="cap"><span class="cap-icon">🎨</span><div><div class="cap-title">Generate</div><div class="cap-desc">Local SDXL / Flux / video via auto-managed ComfyUI.</div></div></div>
        <div class="cap"><span class="cap-icon">🧠</span><div><div class="cap-title">Remember</div><div class="cap-desc">Per-project observations + MCP server tools.</div></div></div>
      </div>
      <div class="launch-actions"></div>
      <div class="launch-hint">Type <kbd>/</kbd> to browse commands · <kbd>⌘ ↵</kbd> to send</div>
    `;
    const actionsEl = launch.querySelector(".launch-actions") as HTMLElement;
    for (const a of actions) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "launch-btn";
      btn.innerHTML = `<span class="launch-btn-label">${escapeHtml(a.label)}</span><span class="launch-btn-desc">${escapeHtml(a.description)}</span>`;
      btn.addEventListener("click", () => a.onActivate?.());
      actionsEl.appendChild(btn);
    }
    container.appendChild(launch);
  };

  const appendStatus = (text: string) => {
    const wrap = document.createElement("div");
    wrap.className = "msg msg-status msg-fade";
    wrap.textContent = text;
    container.appendChild(wrap);
    container.scrollTop = container.scrollHeight;
  };

  return { appendMessage, setMessages, clear, renderLaunch, appendStatus };
}

function renderContent(target: HTMLElement, content: string) {
  target.textContent = "";

  const inlineImages: string[] = [];
  let remaining = content.replace(BASE64_IMG_RE, (match) => {
    inlineImages.push(match);
    return "";
  });

  const localPaths: string[] = [];
  remaining = remaining.replace(PATH_RE, (match, path: string) => {
    if (IMG_EXTENSIONS.test(path)) {
      localPaths.push(path);
      return match.replace(path, "");
    }
    return match;
  });

  const text = document.createElement("span");
  text.textContent = remaining;
  target.appendChild(text);

  for (const src of inlineImages) attachImage(target, src);
  for (const path of localPaths) {
    api
      .readImageAsDataUrl(path)
      .then((dataUrl) => attachImage(target, dataUrl))
      .catch(() => {});
  }
}

function attachImage(target: HTMLElement, src: string) {
  const img = document.createElement("img");
  img.className = "inline";
  img.src = src;
  img.loading = "lazy";
  target.appendChild(img);
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export function formatRelativeTime(timestamp: string): string {
  if (!timestamp) return "";
  const t = Date.parse(timestamp);
  if (Number.isNaN(t)) return timestamp.slice(0, 10);
  const diff = (Date.now() - t) / 1000;
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  return new Date(t).toLocaleDateString();
}
