export type Role = "user" | "assistant" | "system" | "tool";

export interface ChatMessage {
  role: Role;
  content: string;
  tool_calls?: unknown[];
  name?: string;
}

export interface ModelEntry {
  name: string;
  size: number;
  modified_at: string;
}

export interface TranscriptSummary {
  session_id: string;
  timestamp: string;
  model: string;
  project: string;
  messages: number;
  title: string;
  preview: string;
  project_label: string;
}

export interface TranscriptPayload {
  session_id: string;
  model: string;
  project: string;
  timestamp: string;
  messages: ChatMessage[];
}

export interface ConfigPayload {
  schema_version: number;
  default_model: string;
  ollama_models_path: string;
  extra: Record<string, unknown>;
}

export interface SendMessageOutput {
  message: ChatMessage;
  model: string;
  messages: ChatMessage[];
  session_id: string;
}

export interface SlashCommandEntry {
  command: string;
  description: string;
}

export interface UserProfile {
  display_name: string;
  email: string;
  auth_endpoint: string;
  signed_in: boolean;
}

export interface SignInOutput {
  profile: UserProfile;
  token_preview: string | null;
  note: string;
}

export interface McpServerEntry {
  name: string;
  command: string;
  args: string[];
  tool_count: number;
  connected: boolean;
}

export interface MemoryEntry {
  heading: string;
  text: string;
}

export interface TokenUsagePayload {
  used: number;
  limit: number;
  percent: number;
  remaining: number;
  context_window: number;
}

export interface ProjectInfo {
  path: string;
  name: string;
  last_opened: string;
  ecosystems: string[];
}

export interface ProjectScanResult {
  path: string;
  name: string;
  ecosystems: string[];
  has_code: boolean;
}

export interface ActiveProjectState {
  active: ProjectInfo | null;
  recent: ProjectInfo[];
}

export interface DesktopPermissions {
  accessibility: boolean;
  screen_recording: boolean;
  platform: string;
  supported: boolean;
}

export interface FileNode {
  name: string;
  path: string;
  kind: "file" | "dir";
  has_children: boolean;
}

export interface FileContent {
  path: string;
  language: string;
  content: string;
  size_bytes: number;
  truncated: boolean;
}

export interface RegistryModel {
  key: string;
  summary: string;
  size_gb: number;
  use_cases: string[];
  tool_support: boolean;
  vision: boolean;
  source: string; // "ollama" | "gguf" | "comfyui"
  min_ram_gb: number;
  recommended_ram_gb: number;
}
