import { invoke } from "@tauri-apps/api/core";
import type {
  ActiveProjectState,
  ChatMessage,
  ConfigPayload,
  DesktopPermissions,
  FileContent,
  FileNode,
  McpServerEntry,
  MemoryEntry,
  ModelEntry,
  ProjectScanResult,
  RegistryModel,
  SendMessageOutput,
  SignInOutput,
  SlashCommandEntry,
  TokenUsagePayload,
  TranscriptPayload,
  TranscriptSummary,
  UserProfile,
} from "./types";

export const api = {
  listModels: () => invoke<ModelEntry[]>("list_models"),
  activeModel: () => invoke<string>("active_model"),
  setActiveModel: (model: string) => invoke<void>("set_active_model", { model }),
  listTranscripts: (limit = 50) =>
    invoke<TranscriptSummary[]>("list_transcripts", { limit }),
  loadTranscript: (sessionId: string) =>
    invoke<TranscriptPayload>("load_transcript", { session_id: sessionId }),
  sendMessage: (messages: ChatMessage[], model?: string, sessionId?: string) =>
    invoke<SendMessageOutput>("send_message", {
      input: { messages, model: model ?? null, session_id: sessionId ?? null },
    }),
  getConfig: () => invoke<ConfigPayload>("get_config"),
  setConfigField: (key: string, value: unknown) =>
    invoke<ConfigPayload>("set_config_field", { input: { key, value } }),
  readImageAsDataUrl: (path: string) =>
    invoke<string>("read_image_as_data_url", { path }),
  copyImageToClipboard: (path: string) =>
    invoke<void>("copy_image_to_clipboard", { path }),
  saveImageAs: (srcPath: string) =>
    invoke<string | null>("save_image_as", { src_path: srcPath }),
  newSessionId: () => invoke<string>("new_session_id"),
  listSlashCommands: () => invoke<SlashCommandEntry[]>("list_slash_commands"),
  installComfyui: () => invoke<string[]>("install_comfyui_cmd"),
  startComfyui: () => invoke<string>("start_comfyui_cmd"),
  stopComfyui: () => invoke<string>("stop_comfyui_cmd"),
  downloadImageModel: (modelKey: string) =>
    invoke<string[]>("download_image_model_cmd", { model_key: modelKey }),

  getUserProfile: () => invoke<UserProfile>("get_user_profile"),
  saveUserProfile: (profile: UserProfile) =>
    invoke<UserProfile>("save_user_profile", { profile }),
  saveCredential: (account: string, password: string) =>
    invoke<void>("save_credential", { account, password }),
  getCredential: (account: string) =>
    invoke<string | null>("get_credential", { account }),
  deleteCredential: (account: string) =>
    invoke<void>("delete_credential", { account }),
  signIn: (email: string, password: string, remember: boolean) =>
    invoke<SignInOutput>("sign_in", {
      input: { email, password, remember },
    }),
  signUp: (
    email: string,
    password: string,
    firstName: string,
    lastName: string,
    remember: boolean,
  ) =>
    invoke<SignInOutput>("sign_up", {
      input: { email, password, first_name: firstName, last_name: lastName, remember },
    }),
  signOut: () => invoke<UserProfile>("sign_out"),

  listMcpServers: () => invoke<McpServerEntry[]>("list_mcp_servers"),
  addMcpServer: (name: string, command: string, args: string[]) =>
    invoke<void>("add_mcp_server", { input: { name, command, args } }),
  removeMcpServer: (name: string) =>
    invoke<void>("remove_mcp_server", { name }),

  listProjectMemory: (projectPath?: string) =>
    invoke<MemoryEntry[]>("list_project_memory", { project_path: projectPath ?? null }),
  saveObservation: (text: string, scope: "project" | "global" = "project") =>
    invoke<string>("save_observation", { input: { text, scope } }),
  forgetProjectMemory: (index: number) =>
    invoke<boolean>("forget_project_memory", { input: { index } }),

  tokenUsage: (model: string, messages: ChatMessage[]) =>
    invoke<TokenUsagePayload>("token_usage", { input: { model, messages } }),

  runSlash: (command: string, args = "") =>
    invoke<string>("run_slash_text", { input: { command, args } }),

  listModelRegistry: () => invoke<RegistryModel[]>("list_model_registry"),
  pullOllamaModel: (model: string) =>
    invoke<string>("pull_ollama_model", { model }),
  deleteTranscript: (sessionId: string) =>
    invoke<void>("delete_transcript", { session_id: sessionId }),

  getActiveProject: () => invoke<ActiveProjectState>("get_active_project"),
  setActiveProject: (path: string) =>
    invoke<ActiveProjectState>("set_active_project", { path }),
  clearActiveProject: () => invoke<ActiveProjectState>("clear_active_project"),
  forgetRecentProject: (path: string) =>
    invoke<ActiveProjectState>("forget_recent_project", { path }),
  scanProject: (path: string) =>
    invoke<ProjectScanResult>("scan_project", { path }),

  listProjectFiles: (path?: string) =>
    invoke<FileNode[]>("list_project_files", { path: path ?? null }),
  readProjectFile: (path: string) =>
    invoke<FileContent>("read_project_file", { path }),
  writeProjectFile: (path: string, content: string) =>
    invoke<void>("write_project_file", { path, content }),
  drainEditorEvents: () =>
    invoke<{ kind: string; payload: unknown; ts: string }[]>("drain_editor_events"),

  checkDesktopPermissions: () =>
    invoke<DesktopPermissions>("check_desktop_permissions"),
  openSystemSettingsPane: (pane: "accessibility" | "screen_recording" | "automation") =>
    invoke<void>("open_system_settings_pane", { pane }),
};
