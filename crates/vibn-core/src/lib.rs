#[cfg(target_os = "macos")]
mod macos_input;

use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use chrono::Local;
use dirs::home_dir;
use fs2::available_space;
use glob::Pattern;
use reqwest::blocking::Client;
use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, Tool as McpTool},
    service::{RoleClient, RunningService},
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tokio::process::Command as TokioCommand;
use tokio::runtime::Runtime;
use uuid::Uuid;

const DEFAULT_CONFIG_JSON: &str = include_str!("../../../data/default_config.json");
const MODEL_REGISTRY_JSON: &str = include_str!("../../../data/models.json");
const OBSERVATIONS_HEADER: &str = "# Observations\n\n";
const MAX_OBSERVATION_FILE_SIZE: u64 = 50_000;
const MAX_AGENT_ROUNDS: usize = 8;
const CHARS_PER_TOKEN: usize = 4;
const DEFAULT_CONTEXT_WINDOW: usize = 32_768;
const DEFAULT_RUNTIME_CONTEXT_WINDOW: usize = 8_192;
const HOOK_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_OLLAMA_RETRIES: usize = 3;
const OLLAMA_RETRY_BASE_DELAY: Duration = Duration::from_secs(1);

pub const HOOK_SESSION_START: &str = "session_start";
pub const HOOK_PRE_COMPACT: &str = "pre_compact";
pub const HOOK_POST_COMPACT: &str = "post_compact";
pub const HOOK_POST_EDIT: &str = "post_edit";
pub const HOOK_POST_COMMAND: &str = "post_command";
pub const HOOK_PRE_CHAT: &str = "pre_chat";
pub const HOOK_POST_CHAT: &str = "post_chat";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GgufSpec {
    pub url: String,
    pub file: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ComfyuiSpec {
    pub checkpoint: String,
    #[serde(default = "default_comfy_kind")]
    pub kind: String,
    #[serde(default)]
    pub default_steps: Option<u32>,
    #[serde(default)]
    pub default_cfg: Option<f64>,
    #[serde(default)]
    pub default_sampler: Option<String>,
    #[serde(default)]
    pub default_scheduler: Option<String>,
    #[serde(default)]
    pub default_width: Option<u32>,
    #[serde(default)]
    pub default_height: Option<u32>,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub workflow_required: bool,
}

fn default_comfy_kind() -> String {
    "image".to_owned()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelInfo {
    pub summary: String,
    pub size_gb: f64,
    pub use_cases: Vec<String>,
    pub tool_support: bool,
    pub min_ram_gb: u32,
    pub recommended_ram_gb: u32,
    pub source: String,
    #[serde(default)]
    pub gguf: Option<GgufSpec>,
    #[serde(default)]
    pub vision: bool,
    #[serde(default)]
    pub comfyui: Option<ComfyuiSpec>,
}

pub type ModelRegistry = BTreeMap<String, ModelInfo>;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_model")]
    pub default_model: String,
    #[serde(default)]
    pub ollama_models_path: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone)]
pub struct McpServerStatus {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub tool_count: usize,
}

struct McpServerConnection {
    command: String,
    args: Vec<String>,
    client: RunningService<RoleClient, ()>,
    tools: Vec<McpTool>,
}

struct McpManager {
    runtime: Runtime,
    connections: Mutex<HashMap<String, McpServerConnection>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationEntry {
    pub heading: String,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TokenUsage {
    pub used: usize,
    pub limit: usize,
    pub percent: f64,
    pub remaining: usize,
    pub needs_warning: bool,
    pub needs_compact: bool,
    pub context_window: usize,
    pub runtime_context_window: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCall {
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolFunction {
    pub name: String,
    #[serde(default)]
    pub arguments: Map<String, Value>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_owned(),
            content: content.into(),
            tool_calls: Vec::new(),
            name: None,
            images: Vec::new(),
        }
    }

    pub fn user_with_images(content: impl Into<String>, images: Vec<String>) -> Self {
        Self {
            role: "user".to_owned(),
            content: content.into(),
            tool_calls: Vec::new(),
            name: None,
            images,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_owned(),
            content: content.into(),
            tool_calls: Vec::new(),
            name: None,
            images: Vec::new(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_owned(),
            content: content.into(),
            tool_calls: Vec::new(),
            name: None,
            images: Vec::new(),
        }
    }

    pub fn tool(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".to_owned(),
            content: content.into(),
            tool_calls: Vec::new(),
            name: Some(name.into()),
            images: Vec::new(),
        }
    }
}

impl FileStateCache {
    fn get(&mut self, path: &Path) -> Option<String> {
        let (content, cached_mtime) = self.entries.get(path)?.clone();
        let current_mtime = fs::metadata(path).ok()?.modified().ok()?;
        if current_mtime == cached_mtime {
            Some(content)
        } else {
            self.entries.remove(path);
            None
        }
    }

    fn put(&mut self, path: PathBuf, content: String) {
        if let Ok(mtime) = fs::metadata(&path).and_then(|metadata| metadata.modified()) {
            self.entries.insert(path, (content, mtime));
        }
    }

    fn invalidate(&mut self, path: &Path) {
        self.entries.remove(path);
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TranscriptMetadata {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub session_id: String,
    pub timestamp: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone)]
pub struct TranscriptEntry {
    pub session_id: String,
    pub timestamp: String,
    pub model: String,
    pub project: String,
    pub messages: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookResult {
    pub script: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

static MCP_MANAGER: OnceLock<Result<McpManager, String>> = OnceLock::new();

impl McpManager {
    fn new() -> Result<Self, String> {
        let runtime =
            Runtime::new().map_err(|err| format!("failed to start tokio runtime: {err}"))?;
        Ok(Self {
            runtime,
            connections: Mutex::new(HashMap::new()),
        })
    }

    fn connect(
        &self,
        name: &str,
        command: &str,
        args: &[String],
        env_vars: &Map<String, Value>,
    ) -> Result<usize, String> {
        self.disconnect(name)?;
        let name_owned = name.to_owned();
        let command_owned = command.to_owned();
        let args_owned = args.to_vec();
        let env_owned = env_vars.clone();
        let client = self.runtime.block_on(async {
            let child = TokioCommand::new(&command_owned);
            let transport = TokioChildProcess::new(child.configure(|command| {
                command.args(&args_owned);
                for (key, value) in &env_owned {
                    let Some(value) = value.as_str() else {
                        continue;
                    };
                    command.env(key, value);
                }
            }))
            .map_err(|err| format!("failed to start MCP server {name_owned}: {err}"))?;
            ().serve(transport)
                .await
                .map_err(|err| format!("failed to initialize MCP server {name_owned}: {err}"))
        })?;
        let tools = self.runtime.block_on(async {
            client
                .peer()
                .list_all_tools()
                .await
                .map_err(|err| format!("failed to list MCP tools for {name_owned}: {err}"))
        })?;
        let tool_count = tools.len();
        self.connections
            .lock()
            .map_err(|_| "failed to lock MCP connections".to_owned())?
            .insert(
                name_owned,
                McpServerConnection {
                    command: command_owned,
                    args: args_owned,
                    client,
                    tools,
                },
            );
        Ok(tool_count)
    }

    fn disconnect(&self, name: &str) -> Result<(), String> {
        let connection = self
            .connections
            .lock()
            .map_err(|_| "failed to lock MCP connections".to_owned())?
            .remove(name);
        if let Some(connection) = connection {
            self.runtime
                .block_on(async { connection.client.cancel().await })
                .map_err(|err| format!("failed to stop MCP server {name}: {err}"))?;
        }
        Ok(())
    }

    fn statuses(&self) -> Result<Vec<McpServerStatus>, String> {
        let connections = self
            .connections
            .lock()
            .map_err(|_| "failed to lock MCP connections".to_owned())?;
        let mut statuses = connections
            .iter()
            .map(|(name, connection)| McpServerStatus {
                name: name.clone(),
                command: connection.command.clone(),
                args: connection.args.clone(),
                tool_count: connection.tools.len(),
            })
            .collect::<Vec<_>>();
        statuses.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(statuses)
    }

    fn tool_payloads(&self) -> Result<Vec<Value>, String> {
        let connections = self
            .connections
            .lock()
            .map_err(|_| "failed to lock MCP connections".to_owned())?;
        let mut payloads = Vec::new();
        for (server_name, connection) in connections.iter() {
            for tool in &connection.tools {
                payloads.push(mcp_tool_payload(server_name, tool));
            }
        }
        Ok(payloads)
    }

    fn tool_names(&self) -> Result<Vec<String>, String> {
        let connections = self
            .connections
            .lock()
            .map_err(|_| "failed to lock MCP connections".to_owned())?;
        let mut names = Vec::new();
        for (server_name, connection) in connections.iter() {
            for tool in &connection.tools {
                names.push(format!("mcp__{}__{}", server_name, tool.name));
            }
        }
        Ok(names)
    }

    fn connected_summary(&self) -> Result<String, String> {
        let connections = self
            .connections
            .lock()
            .map_err(|_| "failed to lock MCP connections".to_owned())?;
        if connections.is_empty() {
            return Ok(String::new());
        }
        let mut lines = Vec::new();
        let mut names = connections.keys().cloned().collect::<Vec<_>>();
        names.sort();
        for server_name in names {
            let connection = &connections[&server_name];
            let tool_names = connection
                .tools
                .iter()
                .map(|tool| tool.name.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!(
                "- **{}**: {}",
                server_name,
                if tool_names.is_empty() {
                    "no tools".to_owned()
                } else {
                    tool_names
                }
            ));
            for tool in &connection.tools {
                let description = tool
                    .description
                    .as_deref()
                    .unwrap_or("no description")
                    .chars()
                    .take(80)
                    .collect::<String>();
                lines.push(format!(
                    "  - `mcp__{}__{}`: {}",
                    server_name, tool.name, description
                ));
            }
        }
        Ok(lines.join("\n"))
    }

    fn call_tool(&self, full_name: &str, arguments: &Map<String, Value>) -> Result<String, String> {
        let Some((server_name, tool_name)) = parse_mcp_tool_name(full_name) else {
            return Err(format!("unknown MCP tool: {full_name}"));
        };
        let mut connections = self
            .connections
            .lock()
            .map_err(|_| "failed to lock MCP connections".to_owned())?;
        let Some(connection) = connections.get_mut(server_name) else {
            return Err(format!("MCP server {server_name} is not connected"));
        };
        let tool_name = tool_name.to_owned();
        let result = self.runtime.block_on(async {
            connection
                .client
                .peer()
                .call_tool(CallToolRequestParams::new(tool_name).with_arguments(arguments.clone()))
                .await
                .map_err(|err| format!("MCP tool {full_name} failed: {err}"))
        })?;
        Ok(render_mcp_tool_result(&result))
    }
}

fn global_mcp_manager() -> Result<&'static McpManager, String> {
    MCP_MANAGER
        .get_or_init(|| McpManager::new())
        .as_ref()
        .map_err(Clone::clone)
}

fn mcp_tool_payload(server_name: &str, tool: &McpTool) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": format!("mcp__{}__{}", server_name, tool.name),
            "description": tool.description.clone().unwrap_or_default(),
            "parameters": Value::Object((*tool.input_schema).clone()),
        }
    })
}

fn parse_mcp_tool_name(name: &str) -> Option<(&str, &str)> {
    let remainder = name.strip_prefix("mcp__")?;
    let (server_name, tool_name) = remainder.split_once("__")?;
    Some((server_name, tool_name))
}

fn render_mcp_tool_result(result: &rmcp::model::CallToolResult) -> String {
    let mut parts = Vec::new();
    for content in &result.content {
        if let Some(text) = content.raw.as_text() {
            parts.push(text.text.clone());
            continue;
        }
        if let Some(resource) = content.raw.as_resource() {
            parts.push(
                serde_json::to_string_pretty(resource)
                    .unwrap_or_else(|_| "[embedded resource]".to_owned()),
            );
            continue;
        }
        if let Some(link) = content.raw.as_resource_link() {
            parts.push(
                serde_json::to_string_pretty(link).unwrap_or_else(|_| "[resource link]".to_owned()),
            );
            continue;
        }
        parts.push(
            serde_json::to_string_pretty(&content.raw)
                .unwrap_or_else(|_| "[unsupported MCP content]".to_owned()),
        );
    }
    if let Some(structured) = &result.structured_content {
        parts.push(
            serde_json::to_string_pretty(structured).unwrap_or_else(|_| structured.to_string()),
        );
    }
    if parts.is_empty() {
        if result.is_error == Some(true) {
            "MCP tool returned an error with no content.".to_owned()
        } else {
            "MCP tool returned no content.".to_owned()
        }
    } else {
        parts.join("\n\n")
    }
}

type ConfirmCallbackFn = fn(&str, &Map<String, Value>, &str) -> bool;
type DiffCallbackFn = fn(&Path, &str, &str) -> bool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashCommandDefinition {
    pub command: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone)]
pub struct AgentRunResult {
    pub messages: Vec<ChatMessage>,
    pub final_text: String,
    pub auto_compact_summary: Option<String>,
}

#[derive(Debug, Default)]
struct FileStateCache {
    entries: BTreeMap<PathBuf, (String, SystemTime)>,
}

#[derive(Debug)]
pub struct OllamaClient {
    base_url: String,
    client: Client,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemProfile {
    pub system: String,
    pub machine: String,
    pub cpu_count: usize,
    pub total_ram_gb: Option<f64>,
    pub storage_path: PathBuf,
    pub storage_free_gb: Option<f64>,
}

impl SystemProfile {
    pub fn is_apple_silicon(&self) -> bool {
        self.system == "macOS" && matches!(self.machine.as_str(), "arm64" | "aarch64")
    }
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: ChatMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFit {
    Good,
    Tight,
    TooLarge,
    TightDisk,
    Unknown,
}

impl ModelFit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Good => "good",
            Self::Tight => "tight",
            Self::TooLarge => "too large",
            Self::TightDisk => "tight disk",
            Self::Unknown => "unknown",
        }
    }
}

impl AppConfig {
    pub fn command_timeout_secs(&self) -> u64 {
        self.extra
            .get("command_timeout")
            .and_then(|value| value.as_u64())
            .unwrap_or(120)
    }

    pub fn command_frequency(&self, command: &str) -> u64 {
        let base = command.split_whitespace().next().unwrap_or(command).trim();
        self.extra
            .get("command_usage")
            .and_then(Value::as_object)
            .and_then(|usage| usage.get(base))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    }

    pub fn track_command_usage(&mut self, command: &str) {
        let base = command
            .split_whitespace()
            .next()
            .unwrap_or(command)
            .trim()
            .to_lowercase();
        if !base.starts_with('/') {
            return;
        }
        let usage = self
            .extra
            .entry("command_usage".to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(usage) = usage.as_object_mut() {
            let count = usage.get(&base).and_then(Value::as_u64).unwrap_or(0) + 1;
            usage.insert(base, Value::from(count));
        }
    }
}

pub fn builtin_tool_definitions() -> &'static [ToolDefinition] {
    &[
        ToolDefinition {
            name: "read_file",
            description: "Read file contents with numbered lines.",
        },
        ToolDefinition {
            name: "write_file",
            description: "Create or overwrite a file with given content.",
        },
        ToolDefinition {
            name: "edit_file",
            description: "Replace an exact string in a file.",
        },
        ToolDefinition {
            name: "list_directory",
            description: "List files and directories from the working directory.",
        },
        ToolDefinition {
            name: "run_command",
            description: "Execute a shell command in the working directory.",
        },
        ToolDefinition {
            name: "search_code",
            description: "Search code for a text pattern with file:line results.",
        },
        ToolDefinition {
            name: "find_files",
            description: "Find files matching a glob pattern.",
        },
        ToolDefinition {
            name: "patch_file",
            description: "Apply multiple exact-string edits to a file.",
        },
        ToolDefinition {
            name: "git",
            description: "Run a git command in the working directory.",
        },
        ToolDefinition {
            name: "save_observation",
            description: "Save an observation to global or project memory.",
        },
        ToolDefinition {
            name: "read_observations",
            description: "Read saved global and project observations.",
        },
        ToolDefinition {
            name: "read_image",
            description: "Describe or answer questions about a local image file using the configured vision model. Only call this when the user explicitly asks you to look at an image they provided. NEVER call this on images you just generated — the user already sees the rendered output in the UI.",
        },
        ToolDefinition {
            name: "read_video",
            description: "Sample frames from a local video and analyse them with the configured vision model (requires ffmpeg).",
        },
        ToolDefinition {
            name: "generate_image",
            description: "Generate an image from a text prompt via a local ComfyUI server.",
        },
        ToolDefinition {
            name: "generate_video",
            description: "Generate a short video from a text prompt via a local ComfyUI server (requires a workflow template).",
        },
        ToolDefinition {
            name: "install_comfy",
            description: "Install a local managed ComfyUI (clones the repo, creates a Python venv, installs requirements). Takes 5-15 minutes. Ask the user to confirm before running.",
        },
        ToolDefinition {
            name: "download_image_model",
            description: "Download a ComfyUI image-generation checkpoint (e.g. comfyui:sdxl-base, comfyui:flux1-schnell) into the managed install. Multi-GB download.",
        },
        ToolDefinition {
            name: "open_in_editor",
            description: "Open a file in the desktop app's editor panel. Use this when discussing a file so the user can follow along. Has no effect in the CLI.",
        },
        ToolDefinition {
            name: "show_in_explorer",
            description: "Reveal a file or directory in the desktop app's file tree, expanding ancestors as needed. Has no effect in the CLI.",
        },
        ToolDefinition {
            name: "list_windows",
            description: "List visible application windows on the user's desktop (macOS only). Returns app, title, position, size, frontmost flag. Requires the 'desktop use' setting to be enabled.",
        },
        ToolDefinition {
            name: "focus_window",
            description: "Bring an application or window to the front (macOS only). Provide `app_name` or `window_title`. Requires the 'desktop use' setting.",
        },
        ToolDefinition {
            name: "screenshot",
            description: "Capture a screenshot of the entire screen or a region (macOS only). Saves a PNG to ~/.vibn/screenshots/ and returns the path. Pair with read_image to inspect contents.",
        },
        ToolDefinition {
            name: "read_selected_text",
            description: "Read the currently selected text in the frontmost app (macOS only, requires Accessibility permission). Returns the text or empty string.",
        },
        ToolDefinition {
            name: "send_keys",
            description: "Type text or send a keyboard shortcut to the frontmost app (macOS only). Provide `text` for typed characters or `shortcut` like 'cmd+s'. Requires Accessibility permission.",
        },
        ToolDefinition {
            name: "run_applescript",
            description: "Execute an AppleScript snippet via osascript (macOS only). Use sparingly — prefer the higher-level tools when they fit. Timeout default 10s, max 60s.",
        },
        ToolDefinition {
            name: "cursor_position",
            description: "Return the current mouse cursor position as { x, y } in screen pixels (macOS only). Requires the 'desktop use' setting.",
        },
        ToolDefinition {
            name: "mouse_move",
            description: "Move the mouse cursor to (x, y) without clicking (macOS only). Coordinates are in screen pixels, origin top-left. Requires Accessibility permission.",
        },
        ToolDefinition {
            name: "mouse_click",
            description: "Click at screen pixel (x, y). button is 'left' (default), 'right', or 'middle'; clicks=2 for double-click. After clicking the agent will automatically receive a fresh screenshot. Requires Accessibility permission.",
        },
        ToolDefinition {
            name: "mouse_drag",
            description: "Press at (from_x, from_y), drag to (to_x, to_y), release (macOS only). Use for text selection, sliders, and resize handles. After dragging the agent will automatically receive a fresh screenshot.",
        },
        ToolDefinition {
            name: "scroll",
            description: "Synthesise a scroll event by (dx, dy) lines at the current cursor position (macOS only). Positive dy = scroll up (natural scrolling). After scrolling the agent will automatically receive a fresh screenshot.",
        },
    ]
}

pub fn slash_command_definitions() -> &'static [SlashCommandDefinition] {
    &[
        SlashCommandDefinition {
            command: "/help",
            description: "Show all commands",
        },
        SlashCommandDefinition {
            command: "/market",
            description: "Browse MCP marketplace",
        },
        SlashCommandDefinition {
            command: "/skills",
            description: "Browse and activate skills",
        },
        SlashCommandDefinition {
            command: "/skill ",
            description: "Activate a skill by name",
        },
        SlashCommandDefinition {
            command: "/cd ",
            description: "Change working directory",
        },
        SlashCommandDefinition {
            command: "/tree",
            description: "Show project file tree",
        },
        SlashCommandDefinition {
            command: "/git ",
            description: "Run a git command",
        },
        SlashCommandDefinition {
            command: "/mcp",
            description: "Manage MCP servers",
        },
        SlashCommandDefinition {
            command: "/model",
            description: "Switch LLM model",
        },
        SlashCommandDefinition {
            command: "/vision-model ",
            description: "Set the vision model used by read_image / read_video",
        },
        SlashCommandDefinition {
            command: "/image-model ",
            description: "Set the ComfyUI image-generation model",
        },
        SlashCommandDefinition {
            command: "/video-model ",
            description: "Set the ComfyUI video-generation model",
        },
        SlashCommandDefinition {
            command: "/comfy-url ",
            description: "Set the ComfyUI server URL (default http://127.0.0.1:8188)",
        },
        SlashCommandDefinition {
            command: "/install-comfy",
            description: "Install a managed ComfyUI into ~/.vibn/comfyui (git + python venv)",
        },
        SlashCommandDefinition {
            command: "/start-comfy",
            description: "Start the managed ComfyUI server in the background",
        },
        SlashCommandDefinition {
            command: "/stop-comfy",
            description: "Stop the managed ComfyUI server",
        },
        SlashCommandDefinition {
            command: "/download-image-model ",
            description: "Download a ComfyUI checkpoint from the registry into ~/.vibn/comfyui",
        },
        SlashCommandDefinition {
            command: "/model-path",
            description: "Choose model storage path",
        },
        SlashCommandDefinition {
            command: "/models",
            description: "Recommend models for this machine",
        },
        SlashCommandDefinition {
            command: "/perf",
            description: "Check local model performance fit",
        },
        SlashCommandDefinition {
            command: "/status",
            description: "Show session info",
        },
        SlashCommandDefinition {
            command: "/config",
            description: "Show or set config",
        },
        SlashCommandDefinition {
            command: "/resume ",
            description: "Resume a previous session",
        },
        SlashCommandDefinition {
            command: "/plan",
            description: "Toggle plan-before-act mode",
        },
        SlashCommandDefinition {
            command: "/allow ",
            description: "Allow a tool/command permanently",
        },
        SlashCommandDefinition {
            command: "/deny ",
            description: "Block a tool/command permanently",
        },
        SlashCommandDefinition {
            command: "/compact",
            description: "Compress conversation",
        },
        SlashCommandDefinition {
            command: "/tokens",
            description: "Show token usage",
        },
        SlashCommandDefinition {
            command: "/bg ",
            description: "Run a task in the background",
        },
        SlashCommandDefinition {
            command: "/tasks",
            description: "List background tasks",
        },
        SlashCommandDefinition {
            command: "/test ",
            description: "Run tests, auto-fix on failure",
        },
        SlashCommandDefinition {
            command: "/transcripts",
            description: "List saved transcripts",
        },
        SlashCommandDefinition {
            command: "/sessions",
            description: "List named sessions",
        },
        SlashCommandDefinition {
            command: "/remember ",
            description: "Save a fact to agent memory",
        },
        SlashCommandDefinition {
            command: "/memory",
            description: "Show remembered facts for this project",
        },
        SlashCommandDefinition {
            command: "/forget ",
            description: "Remove a memory by number",
        },
        SlashCommandDefinition {
            command: "/commit",
            description: "Generate and apply a git commit",
        },
        SlashCommandDefinition {
            command: "/reset",
            description: "Clear conversation",
        },
        SlashCommandDefinition {
            command: "/diff",
            description: "Show git diff (rendered)",
        },
        SlashCommandDefinition {
            command: "/undo",
            description: "Remove last message turn",
        },
        SlashCommandDefinition {
            command: "/export-training-data",
            description: "Export sessions for fine-tuning",
        },
        SlashCommandDefinition {
            command: "/generate-training-data",
            description: "Generate synthetic training examples",
        },
        SlashCommandDefinition {
            command: "/review-training",
            description: "Review and approve generated training examples",
        },
        SlashCommandDefinition {
            command: "/constraints",
            description: "View/add/remove generation constraints",
        },
        SlashCommandDefinition {
            command: "/search ",
            description: "Search file contents with ripgrep",
        },
        SlashCommandDefinition {
            command: "/find ",
            description: "Find files by name pattern",
        },
        SlashCommandDefinition {
            command: "/compare ",
            description: "Run a prompt on two models and compare responses",
        },
        SlashCommandDefinition {
            command: "/prompts",
            description: "List saved prompt templates",
        },
        SlashCommandDefinition {
            command: "/prompt-save ",
            description: "Save current or given prompt as a template",
        },
        SlashCommandDefinition {
            command: "/prompt-run ",
            description: "Run a saved prompt template",
        },
        SlashCommandDefinition {
            command: "/watch ",
            description: "Watch a path and trigger agent on changes",
        },
        SlashCommandDefinition {
            command: "/unwatch",
            description: "Stop file watcher",
        },
        SlashCommandDefinition {
            command: "/clip",
            description: "Copy last agent response to clipboard",
        },
        SlashCommandDefinition {
            command: "/open ",
            description: "Open a file in $EDITOR",
        },
        SlashCommandDefinition {
            command: "/pin ",
            description: "Pin a note that survives /compact",
        },
        SlashCommandDefinition {
            command: "/pins",
            description: "Show pinned notes",
        },
        SlashCommandDefinition {
            command: "/quit",
            description: "Exit Vibn",
        },
    ]
}

pub fn builtin_tool_payloads() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{"name":"read_file","description":"Read file contents with numbered lines.","parameters":{"type":"object","properties":{"path":{"type":"string"},"offset":{"type":"integer"},"limit":{"type":"integer"}},"required":["path"]}}}),
        json!({"type":"function","function":{"name":"write_file","description":"Create or overwrite a file with given content.","parameters":{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]}}}),
        json!({"type":"function","function":{"name":"edit_file","description":"Replace an exact string in a file.","parameters":{"type":"object","properties":{"path":{"type":"string"},"old_string":{"type":"string"},"new_string":{"type":"string"}},"required":["path","old_string","new_string"]}}}),
        json!({"type":"function","function":{"name":"list_directory","description":"List files and directories from the working directory.","parameters":{"type":"object","properties":{"path":{"type":"string"},"recursive":{"type":"boolean"},"depth":{"type":"integer"}},"required":[]}}}),
        json!({"type":"function","function":{"name":"run_command","description":"Execute a shell command in the working directory.","parameters":{"type":"object","properties":{"command":{"type":"string"},"working_dir":{"type":"string"}},"required":["command"]}}}),
        json!({"type":"function","function":{"name":"search_code","description":"Search code for a text pattern with file:line results.","parameters":{"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string"},"file_pattern":{"type":"string"}},"required":["pattern"]}}}),
        json!({"type":"function","function":{"name":"find_files","description":"Find files matching a glob pattern.","parameters":{"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string"}},"required":["pattern"]}}}),
        json!({"type":"function","function":{"name":"patch_file","description":"Apply multiple exact-string edits to a file.","parameters":{"type":"object","properties":{"path":{"type":"string"},"edits":{"type":"array","items":{"type":"object","properties":{"old":{"type":"string"},"new":{"type":"string"}},"required":["old","new"]}}},"required":["path","edits"]}}}),
        json!({"type":"function","function":{"name":"git","description":"Run a git command in the working directory.","parameters":{"type":"object","properties":{"args":{"type":"string"}},"required":["args"]}}}),
        json!({"type":"function","function":{"name":"save_observation","description":"Save an observation to global or project memory.","parameters":{"type":"object","properties":{"text":{"type":"string"},"scope":{"type":"string","enum":["project","global"]}},"required":["text"]}}}),
        json!({"type":"function","function":{"name":"read_observations","description":"Read saved global and project observations.","parameters":{"type":"object","properties":{},"required":[]}}}),
        json!({"type":"function","function":{"name":"read_image","description":"Describe or answer questions about a local image file the USER explicitly provided. NEVER call this on images you generated yourself — the user already sees the rendered output.","parameters":{"type":"object","properties":{"path":{"type":"string","description":"Image file path (jpg/png/webp/gif)"},"prompt":{"type":"string","description":"Optional question; defaults to 'Describe this image.'"},"model":{"type":"string","description":"Override the vision model for this call."}},"required":["path"]}}}),
        json!({"type":"function","function":{"name":"read_video","description":"Sample frames from a local video and analyse them with the configured vision model.","parameters":{"type":"object","properties":{"path":{"type":"string"},"prompt":{"type":"string"},"frames":{"type":"integer","description":"Number of frames to sample (default 6)"},"model":{"type":"string"}},"required":["path"]}}}),
        json!({"type":"function","function":{"name":"generate_image","description":"Generate an image from a text prompt via a local ComfyUI server.","parameters":{"type":"object","properties":{"prompt":{"type":"string"},"negative":{"type":"string"},"width":{"type":"integer"},"height":{"type":"integer"},"steps":{"type":"integer"},"cfg":{"type":"number"},"seed":{"type":"integer"},"sampler":{"type":"string"},"scheduler":{"type":"string"},"model":{"type":"string","description":"Override the image_gen_model registry key."},"output_path":{"type":"string","description":"Where to save the generated image (defaults to ~/.vibn/generated/)"}},"required":["prompt"]}}}),
        json!({"type":"function","function":{"name":"generate_video","description":"Generate a short video from a text prompt via a local ComfyUI server.","parameters":{"type":"object","properties":{"prompt":{"type":"string"},"negative":{"type":"string"},"width":{"type":"integer"},"height":{"type":"integer"},"steps":{"type":"integer"},"frames":{"type":"integer"},"seed":{"type":"integer"},"model":{"type":"string"},"workflow_template":{"type":"string","description":"Path to a ComfyUI API-format workflow JSON template. Use {prompt}, {negative}, {seed}, {steps}, {width}, {height}, {frames} placeholders."},"output_path":{"type":"string"}},"required":["prompt"]}}}),
        json!({"type":"function","function":{"name":"install_comfy","description":"Install local managed ComfyUI. Takes 5-15 minutes. Ask the user to confirm before calling.","parameters":{"type":"object","properties":{},"required":[]}}}),
        json!({"type":"function","function":{"name":"download_image_model","description":"Download a ComfyUI checkpoint into the managed install. Multi-GB.","parameters":{"type":"object","properties":{"model":{"type":"string","description":"Registry key, e.g. comfyui:sdxl-base or comfyui:flux1-schnell"}},"required":["model"]}}}),
        json!({"type":"function","function":{"name":"open_in_editor","description":"Open a project file in the desktop app's editor panel so the user can see it. No-op in CLI.","parameters":{"type":"object","properties":{"path":{"type":"string"},"focus":{"type":"boolean","description":"If true, also focus the editor panel."}},"required":["path"]}}}),
        json!({"type":"function","function":{"name":"show_in_explorer","description":"Reveal a file or directory in the desktop file tree, expanding parent folders. No-op in CLI.","parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}}}),
        json!({"type":"function","function":{"name":"list_windows","description":"List visible app windows on the user's desktop (macOS).","parameters":{"type":"object","properties":{},"required":[]}}}),
        json!({"type":"function","function":{"name":"focus_window","description":"Bring an app or window to the front (macOS). One of app_name or window_title must be provided.","parameters":{"type":"object","properties":{"app_name":{"type":"string"},"window_title":{"type":"string"}},"required":[]}}}),
        json!({"type":"function","function":{"name":"screenshot","description":"Capture a PNG screenshot. region defaults to 'screen'; pass {x,y,w,h} for a region.","parameters":{"type":"object","properties":{"region":{"description":"'screen' | { x, y, w, h }","oneOf":[{"type":"string","enum":["screen"]},{"type":"object","properties":{"x":{"type":"integer"},"y":{"type":"integer"},"w":{"type":"integer"},"h":{"type":"integer"}},"required":["x","y","w","h"]}]}},"required":[]}}}),
        json!({"type":"function","function":{"name":"read_selected_text","description":"Read the currently selected text in the frontmost app (macOS, requires Accessibility).","parameters":{"type":"object","properties":{},"required":[]}}}),
        json!({"type":"function","function":{"name":"send_keys","description":"Type text or send a keyboard shortcut to the frontmost app. Provide `text` OR `shortcut` (e.g. 'cmd+s', 'ctrl+shift+p').","parameters":{"type":"object","properties":{"text":{"type":"string"},"shortcut":{"type":"string"}},"required":[]}}}),
        json!({"type":"function","function":{"name":"run_applescript","description":"Execute an AppleScript via osascript (macOS).","parameters":{"type":"object","properties":{"script":{"type":"string"},"timeout_s":{"type":"integer","description":"Optional timeout in seconds (default 10, max 60)."}},"required":["script"]}}}),
        json!({"type":"function","function":{"name":"cursor_position","description":"Return the current mouse cursor position { x, y } in screen pixels.","parameters":{"type":"object","properties":{},"required":[]}}}),
        json!({"type":"function","function":{"name":"mouse_move","description":"Move the mouse cursor to (x, y) screen pixels without clicking.","parameters":{"type":"object","properties":{"x":{"type":"number"},"y":{"type":"number"}},"required":["x","y"]}}}),
        json!({"type":"function","function":{"name":"mouse_click","description":"Click at screen pixel (x, y). button='left'|'right'|'middle' (default left). clicks=2 for double-click. The agent receives a fresh screenshot after the click lands.","parameters":{"type":"object","properties":{"x":{"type":"number"},"y":{"type":"number"},"button":{"type":"string","enum":["left","right","middle"]},"clicks":{"type":"integer","minimum":1,"maximum":3}},"required":["x","y"]}}}),
        json!({"type":"function","function":{"name":"mouse_drag","description":"Press at (from_x, from_y), drag to (to_x, to_y), release. Defaults to left button.","parameters":{"type":"object","properties":{"from_x":{"type":"number"},"from_y":{"type":"number"},"to_x":{"type":"number"},"to_y":{"type":"number"},"button":{"type":"string","enum":["left","right","middle"]}},"required":["from_x","from_y","to_x","to_y"]}}}),
        json!({"type":"function","function":{"name":"scroll","description":"Scroll by (dx, dy) lines at the current cursor position. Positive dy = scroll up (natural scrolling).","parameters":{"type":"object","properties":{"dx":{"type":"integer"},"dy":{"type":"integer"}},"required":["dy"]}}}),
    ]
}

/// Names of tools gated behind the desktop-use setting. When
/// `config.extra.enable_desktop_tools` is false, these are filtered
/// out of the payload sent to the LLM.
pub const DESKTOP_USE_TOOLS: &[&str] = &[
    "list_windows",
    "focus_window",
    "screenshot",
    "read_selected_text",
    "send_keys",
    "run_applescript",
    "cursor_position",
    "mouse_move",
    "mouse_click",
    "mouse_drag",
    "scroll",
];

/// Computer-use tools whose effect is visible on-screen. After one of these
/// runs the agent loop auto-captures a screenshot and feeds it back into the
/// conversation so the vision model can see the new state without the
/// caller having to chain a separate `screenshot` + `read_image`.
pub const COMPUTER_USE_MUTATING_TOOLS: &[&str] = &[
    "mouse_click",
    "mouse_drag",
    "scroll",
    "send_keys",
    "focus_window",
];

/// Build the tool payload list with config-aware filtering. UI/agent loop
/// should prefer this over `all_tool_payloads()` raw.
pub fn tool_payloads_for_config(config: &AppConfig) -> Vec<Value> {
    let enabled = config
        .extra
        .get("enable_desktop_tools")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    all_tool_payloads()
        .into_iter()
        .filter(|payload| {
            if enabled {
                return true;
            }
            let name = payload
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("");
            !DESKTOP_USE_TOOLS.contains(&name)
        })
        .collect()
}

pub fn all_tool_payloads() -> Vec<Value> {
    let mut payloads = builtin_tool_payloads();
    if let Ok(manager) = global_mcp_manager() {
        if let Ok(mut mcp_payloads) = manager.tool_payloads() {
            payloads.append(&mut mcp_payloads);
        }
    }
    payloads
}

pub fn connect_mcp_server(
    name: &str,
    command: &str,
    args: &[String],
    env_vars: &Map<String, Value>,
) -> Result<usize, String> {
    global_mcp_manager()?.connect(name, command, args, env_vars)
}

pub fn disconnect_mcp_server(name: &str) -> Result<(), String> {
    global_mcp_manager()?.disconnect(name)
}

pub fn list_connected_mcp_servers() -> Result<Vec<McpServerStatus>, String> {
    global_mcp_manager()?.statuses()
}

pub fn connected_mcp_summary() -> String {
    global_mcp_manager()
        .and_then(|manager| manager.connected_summary())
        .unwrap_or_default()
}

pub fn sync_mcp_servers_from_config(config: &AppConfig) -> Result<(), String> {
    let Some(servers) = config.extra.get("mcp_servers").and_then(Value::as_object) else {
        return Ok(());
    };
    for (name, server) in servers {
        let Some(server) = server.as_object() else {
            continue;
        };
        if !server
            .get("auto_connect")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            continue;
        }
        let Some(command) = server.get("command").and_then(Value::as_str) else {
            continue;
        };
        let args = server
            .get("args")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let env_vars = server
            .get("env")
            .or_else(|| server.get("env_vars"))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        connect_mcp_server(name, command, &args, &env_vars)?;
    }
    Ok(())
}

fn parse_tool_calls_from_text(text: &str) -> Vec<ToolCall> {
    let mut tool_names = builtin_tool_definitions()
        .iter()
        .map(|tool| tool.name.to_owned())
        .collect::<Vec<_>>();
    if let Ok(manager) = global_mcp_manager() {
        if let Ok(mut mcp_names) = manager.tool_names() {
            tool_names.append(&mut mcp_names);
        }
    }
    let mut results = Vec::new();
    let bytes = text.as_bytes();

    for (start, ch) in bytes.iter().enumerate() {
        if *ch != b'{' {
            continue;
        }
        let mut depth = 0usize;
        for end in start..bytes.len() {
            match bytes[end] {
                b'{' => depth += 1,
                b'}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        let slice = &text[start..=end];
                        if let Ok(value) = serde_json::from_str::<Value>(slice) {
                            if let Some(tool_call) = value_to_tool_call(&value, &tool_names) {
                                results.push(tool_call);
                            }
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    if results.is_empty() {
        for tool_name in &tool_names {
            for (slice, object) in extract_json_objects(text) {
                let prefix = text[..slice.start].trim_end();
                if prefix.ends_with(tool_name) {
                    if let Some(arguments) = object.as_object() {
                        results.push(ToolCall {
                            function: ToolFunction {
                                name: (*tool_name).to_owned(),
                                arguments: arguments.clone(),
                            },
                        });
                    }
                }
            }
        }
    }

    results
}

fn extract_json_objects(text: &str) -> Vec<(std::ops::Range<usize>, Value)> {
    let mut objects = Vec::new();
    let bytes = text.as_bytes();
    for (start, ch) in bytes.iter().enumerate() {
        if *ch != b'{' {
            continue;
        }
        let mut depth = 0usize;
        for end in start..bytes.len() {
            match bytes[end] {
                b'{' => depth += 1,
                b'}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        let slice = &text[start..=end];
                        if let Ok(value) = serde_json::from_str::<Value>(slice) {
                            objects.push((start..end + 1, value));
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    objects
}

fn value_to_tool_call(value: &Value, tool_names: &[String]) -> Option<ToolCall> {
    let object = value.as_object()?;
    let name = object.get("name")?.as_str()?;
    if !tool_names.iter().any(|tool_name| tool_name == name) {
        return None;
    }
    let arguments = object.get("arguments")?.as_object()?.clone();
    Some(ToolCall {
        function: ToolFunction {
            name: name.to_owned(),
            arguments,
        },
    })
}

pub fn execute_tool(
    name: &str,
    args: &Map<String, Value>,
    config: &AppConfig,
    cwd: &Path,
) -> Result<String, String> {
    let mut no_confirm: Option<&mut ConfirmCallbackFn> = None;
    let mut no_diff: Option<&mut DiffCallbackFn> = None;
    execute_tool_with_callbacks(name, args, config, cwd, &mut no_confirm, &mut no_diff)
}

pub fn execute_tool_with_callbacks<Confirm, Diff>(
    name: &str,
    args: &Map<String, Value>,
    config: &AppConfig,
    cwd: &Path,
    confirm_callback: &mut Option<&mut Confirm>,
    diff_callback: &mut Option<&mut Diff>,
) -> Result<String, String>
where
    Confirm: FnMut(&str, &Map<String, Value>, &str) -> bool,
    Diff: FnMut(&Path, &str, &str) -> bool,
{
    check_tool_permission(name, args, config, confirm_callback)?;
    if name.starts_with("mcp__") {
        return global_mcp_manager()?.call_tool(name, args);
    }
    match name {
        "read_file" => execute_read_file(args, cwd),
        "write_file" => execute_write_file(args, cwd, diff_callback),
        "edit_file" => execute_edit_file(args, cwd, diff_callback),
        "list_directory" => execute_list_directory(args, cwd),
        "run_command" => execute_run_command(args, cwd, config.command_timeout_secs()),
        "search_code" => execute_search_code(args, cwd),
        "find_files" => execute_find_files(args, cwd),
        "patch_file" => execute_patch_file(args, cwd, diff_callback),
        "git" => execute_git(args, cwd),
        "save_observation" => execute_save_observation(args, cwd),
        "read_observations" => execute_read_observations(cwd),
        "read_image" => execute_read_image(args, config, cwd),
        "read_video" => execute_read_video(args, config, cwd),
        "generate_image" => execute_generate_image(args, config, cwd),
        "generate_video" => execute_generate_video(args, config, cwd),
        "install_comfy" => execute_install_comfy(),
        "download_image_model" => execute_download_image_model(args),
        "open_in_editor" => execute_open_in_editor(args, cwd),
        "show_in_explorer" => execute_show_in_explorer(args, cwd),
        "list_windows" => execute_list_windows(args, config),
        "focus_window" => execute_focus_window(args, config),
        "screenshot" => execute_screenshot(args, config),
        "read_selected_text" => execute_read_selected_text(args, config),
        "send_keys" => execute_send_keys(args, config),
        "run_applescript" => execute_run_applescript(args, config),
        "cursor_position" => execute_cursor_position(config),
        "mouse_move" => execute_mouse_move(args, config),
        "mouse_click" => execute_mouse_click(args, config),
        "mouse_drag" => execute_mouse_drag(args, config),
        "scroll" => execute_scroll(args, config),
        _ => Err(format!("Unknown tool: {name}")),
    }
}

// ============================================================================
// Desktop-use tools (Phase 3, VBN-329..334). macOS-first via osascript +
// /usr/sbin/screencapture. Stubs return a clean error on other OSes.
// ============================================================================

fn desktop_use_enabled(config: &AppConfig) -> bool {
    config
        .extra
        .get("enable_desktop_tools")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn require_desktop_use(config: &AppConfig) -> Result<(), String> {
    if desktop_use_enabled(config) {
        Ok(())
    } else {
        Err("desktop_use_disabled: enable 'Allow Vibn to see and control other apps' in Settings → Desktop first.".into())
    }
}

fn escape_applescript_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "macos")]
fn run_osascript(script: &str, timeout: std::time::Duration) -> Result<String, String> {
    use std::process::{Command, Stdio};
    let mut child = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn osascript: {e}"))?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(mut s) = child.stdout.take() {
                    use std::io::Read;
                    let _ = s.read_to_string(&mut stdout);
                }
                if let Some(mut s) = child.stderr.take() {
                    use std::io::Read;
                    let _ = s.read_to_string(&mut stderr);
                }
                if status.success() {
                    return Ok(stdout);
                }
                let msg = if !stderr.trim().is_empty() {
                    stderr.trim().to_owned()
                } else {
                    format!("osascript exited with status {status:?}")
                };
                if msg.to_lowercase().contains("not authorized")
                    || msg.contains("(-1743)")
                    || msg.contains("(-25211)")
                {
                    return Err(format!("permission_denied: {msg}"));
                }
                return Err(msg);
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err("osascript timed out".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn run_osascript(_script: &str, _timeout: std::time::Duration) -> Result<String, String> {
    Err("desktop_use_unsupported: this platform doesn't support AppleScript".into())
}

pub fn execute_list_windows(_args: &Map<String, Value>, config: &AppConfig) -> Result<String, String> {
    require_desktop_use(config)?;
    #[cfg(target_os = "macos")]
    {
        let script = r#"set out to ""
tell application "System Events"
  set procs to (every process whose visible is true)
  repeat with p in procs
    try
      set pname to name of p
      set isFront to (frontmost of p)
      set wins to (windows of p)
      repeat with w in wins
        try
          set wname to name of w
          set wpos to position of w
          set wsize to size of w
          set out to out & pname & "\t" & wname & "\t" & (item 1 of wpos) & "\t" & (item 2 of wpos) & "\t" & (item 1 of wsize) & "\t" & (item 2 of wsize) & "\t" & isFront & "\n"
        end try
      end repeat
    end try
  end repeat
end tell
return out"#;
        let out = run_osascript(script, std::time::Duration::from_secs(8))?;
        let mut windows = Vec::new();
        for line in out.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 7 {
                continue;
            }
            windows.push(serde_json::json!({
                "app": parts[0],
                "title": parts[1],
                "x": parts[2].parse::<i32>().unwrap_or(0),
                "y": parts[3].parse::<i32>().unwrap_or(0),
                "w": parts[4].parse::<i32>().unwrap_or(0),
                "h": parts[5].parse::<i32>().unwrap_or(0),
                "frontmost": parts[6].trim() == "true",
            }));
        }
        Ok(serde_json::to_string_pretty(&windows).unwrap_or_else(|_| "[]".to_owned()))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("desktop_use_unsupported: list_windows is currently macOS-only.".into())
    }
}

pub fn execute_focus_window(args: &Map<String, Value>, config: &AppConfig) -> Result<String, String> {
    require_desktop_use(config)?;
    let app_name = args.get("app_name").and_then(Value::as_str);
    let window_title = args.get("window_title").and_then(Value::as_str);
    if app_name.is_none() && window_title.is_none() {
        return Err("provide app_name or window_title".into());
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(app) = app_name {
            let app_esc = escape_applescript_string(app);
            let script = format!(
                "tell application \"{app_esc}\" to activate\n\
                 tell application \"System Events\" to set frontmost of process \"{app_esc}\" to true",
            );
            run_osascript(&script, std::time::Duration::from_secs(5))?;
            if let Some(title) = window_title {
                let title_esc = escape_applescript_string(title);
                let raise = format!(
                    "tell application \"System Events\" to tell process \"{app_esc}\"\n\
                       set tw to first window whose name contains \"{title_esc}\"\n\
                       perform action \"AXRaise\" of tw\n\
                     end tell",
                );
                let _ = run_osascript(&raise, std::time::Duration::from_secs(5));
            }
            return Ok(format!(
                "Focused {app}{}",
                window_title.map(|t| format!(" ({t})")).unwrap_or_default()
            ));
        }
        // Only window_title supplied: search across all visible procs.
        let title_esc = escape_applescript_string(window_title.unwrap());
        let script = format!(
            "tell application \"System Events\"\n\
               repeat with p in (every process whose visible is true)\n\
                 try\n\
                   set tw to first window of p whose name contains \"{title_esc}\"\n\
                   set frontmost of p to true\n\
                   perform action \"AXRaise\" of tw\n\
                   return name of p\n\
                 end try\n\
               end repeat\n\
             end tell\n\
             return \"\"",
        );
        let out = run_osascript(&script, std::time::Duration::from_secs(6))?;
        let trimmed = out.trim();
        if trimmed.is_empty() {
            Err(format!("no window with title containing '{}' found", window_title.unwrap()))
        } else {
            Ok(format!("Focused window in {trimmed}"))
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app_name, window_title);
        Err("desktop_use_unsupported: focus_window is currently macOS-only.".into())
    }
}

#[cfg(target_os = "macos")]
fn vibn_screenshots_dir() -> PathBuf {
    let dir = vibn_config_dir().join("screenshots");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn execute_screenshot(args: &Map<String, Value>, config: &AppConfig) -> Result<String, String> {
    require_desktop_use(config)?;
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let out_path = vibn_screenshots_dir().join(format!("vibn-{ts}.png"));
        let mut cmd = Command::new("/usr/sbin/screencapture");
        cmd.arg("-x"); // no sound

        let region = args.get("region");
        match region {
            Some(Value::Object(obj)) => {
                let x = obj.get("x").and_then(Value::as_i64).ok_or("region.x missing")?;
                let y = obj.get("y").and_then(Value::as_i64).ok_or("region.y missing")?;
                let w = obj.get("w").and_then(Value::as_i64).ok_or("region.w missing")?;
                let h = obj.get("h").and_then(Value::as_i64).ok_or("region.h missing")?;
                cmd.arg("-R").arg(format!("{x},{y},{w},{h}"));
            }
            Some(Value::String(s)) if s != "screen" => {
                return Err(format!("unsupported region: {s} (use 'screen' or {{x,y,w,h}})"));
            }
            _ => { /* full screen */ }
        }
        cmd.arg(&out_path);
        let status = cmd.status().map_err(|e| format!("failed to run screencapture: {e}"))?;
        if !status.success() {
            return Err(format!(
                "screencapture exited with status {status:?} — likely missing Screen Recording permission"
            ));
        }
        Ok(format!("Saved screenshot to {}", out_path.display()))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = args;
        Err("desktop_use_unsupported: screenshot is currently macOS-only.".into())
    }
}

pub fn execute_read_selected_text(
    _args: &Map<String, Value>,
    config: &AppConfig,
) -> Result<String, String> {
    require_desktop_use(config)?;
    #[cfg(target_os = "macos")]
    {
        // Clipboard save → cmd+c → read pasteboard → restore. Best-effort.
        let script = r#"set savedClipboard to ""
try
  set savedClipboard to (the clipboard as text)
end try
set the clipboard to ""
tell application "System Events" to keystroke "c" using {command down}
delay 0.15
set out to ""
try
  set out to (the clipboard as text)
end try
try
  set the clipboard to savedClipboard
end try
return out"#;
        let out = run_osascript(script, std::time::Duration::from_secs(4))?;
        Ok(out.trim_end_matches('\n').to_owned())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("desktop_use_unsupported: read_selected_text is currently macOS-only.".into())
    }
}

pub fn execute_send_keys(args: &Map<String, Value>, config: &AppConfig) -> Result<String, String> {
    require_desktop_use(config)?;
    #[cfg(target_os = "macos")]
    {
        let text = args.get("text").and_then(Value::as_str);
        let shortcut = args.get("shortcut").and_then(Value::as_str);
        if text.is_none() && shortcut.is_none() {
            return Err("provide either text or shortcut".into());
        }
        let script = if let Some(t) = text {
            let t_esc = escape_applescript_string(t);
            format!("tell application \"System Events\" to keystroke \"{t_esc}\"")
        } else {
            let s = shortcut.unwrap();
            // shortcut like "cmd+shift+s" or "ctrl+a"
            let parts: Vec<&str> = s.split('+').map(str::trim).collect();
            let Some(key) = parts.last() else {
                return Err("empty shortcut".into());
            };
            let mods: Vec<&str> = parts[..parts.len() - 1].to_vec();
            let mut down: Vec<&str> = Vec::new();
            for m in &mods {
                match m.to_lowercase().as_str() {
                    "cmd" | "command" | "meta" => down.push("command down"),
                    "ctrl" | "control" => down.push("control down"),
                    "alt" | "option" | "opt" => down.push("option down"),
                    "shift" => down.push("shift down"),
                    other => return Err(format!("unknown modifier: {other}")),
                }
            }
            let key_esc = escape_applescript_string(key);
            if down.is_empty() {
                format!("tell application \"System Events\" to keystroke \"{key_esc}\"")
            } else {
                let mods_str = down.join(", ");
                format!(
                    "tell application \"System Events\" to keystroke \"{key_esc}\" using {{{mods_str}}}"
                )
            }
        };
        run_osascript(&script, std::time::Duration::from_secs(4))?;
        Ok("ok".into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = args;
        Err("desktop_use_unsupported: send_keys is currently macOS-only.".into())
    }
}

pub fn execute_run_applescript(
    args: &Map<String, Value>,
    config: &AppConfig,
) -> Result<String, String> {
    require_desktop_use(config)?;
    #[cfg(target_os = "macos")]
    {
        let script = args
            .get("script")
            .and_then(Value::as_str)
            .ok_or("missing script")?;
        let timeout = args
            .get("timeout_s")
            .and_then(Value::as_u64)
            .unwrap_or(10)
            .min(60);
        run_osascript(script, std::time::Duration::from_secs(timeout))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = args;
        Err("desktop_use_unsupported: run_applescript is currently macOS-only.".into())
    }
}

/// Result of an auto-screenshot after a mutating computer-use tool.
/// `description` comes from the configured vision model, `image_b64` is the
/// raw PNG so vision-capable agents can also see it directly.
struct LoopScreenshot {
    description: String,
    image_b64: String,
}

/// Capture the screen, run it through the vision model for a UI-focused
/// description, and return both. Returns `None` on any failure — we never
/// want a flaky capture or a missing vision model to abort the agent turn.
#[cfg(target_os = "macos")]
fn capture_screenshot_for_loop(config: &AppConfig) -> Option<LoopScreenshot> {
    use base64::Engine;
    // Give the UI a beat to paint the new state before we capture.
    std::thread::sleep(std::time::Duration::from_millis(250));
    let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S-%f").to_string();
    let path = vibn_screenshots_dir().join(format!("loop-{ts}.png"));
    let status = std::process::Command::new("/usr/sbin/screencapture")
        .arg("-x")
        .arg(&path)
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    let image_b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

    // Describe the screen so non-vision agents can act on it. We deliberately
    // keep the prompt UI-focused — bounding-box estimates over poetry.
    let vision_model = vision_model_for(config);
    let prompt = "You are the eyes of an autonomous desktop agent. Describe \
        what is currently on screen so the agent can decide its next action. \
        Be specific about: the active window, visible buttons / inputs / \
        menus and their approximate screen-pixel positions, any modal dialog \
        or notification, and any text that's clearly readable. Keep it under \
        180 words.";
    let description = match vision_chat(&vision_model, prompt, vec![image_b64.clone()]) {
        Ok(text) => text,
        Err(err) => format!("(vision describe failed: {err})"),
    };
    Some(LoopScreenshot {
        description,
        image_b64,
    })
}

#[cfg(not(target_os = "macos"))]
fn capture_screenshot_for_loop(_config: &AppConfig) -> Option<LoopScreenshot> {
    None
}

// ===== Computer-use mouse primitives =====================================
//
// All five tools are macOS-only and gated by the same desktop-use setting as
// screenshot / send_keys. The agent loop pairs them with an auto-screenshot
// when they belong to COMPUTER_USE_MUTATING_TOOLS so the vision model can
// see what changed.

#[cfg(target_os = "macos")]
fn parse_button(args: &Map<String, Value>) -> Result<macos_input::MouseButton, String> {
    use macos_input::MouseButton;
    match args.get("button").and_then(Value::as_str).unwrap_or("left") {
        "left" => Ok(MouseButton::Left),
        "right" => Ok(MouseButton::Right),
        "middle" => Ok(MouseButton::Middle),
        other => Err(format!("unknown button: {other} (use left|right|middle)")),
    }
}

#[cfg(target_os = "macos")]
fn need_f64(args: &Map<String, Value>, key: &str) -> Result<f64, String> {
    args.get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("missing or non-numeric '{key}'"))
}

pub fn execute_cursor_position(config: &AppConfig) -> Result<String, String> {
    require_desktop_use(config)?;
    #[cfg(target_os = "macos")]
    {
        let (x, y) = macos_input::cursor_position()?;
        Ok(json!({ "x": x, "y": y }).to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("desktop_use_unsupported: cursor_position is currently macOS-only.".into())
    }
}

pub fn execute_mouse_move(args: &Map<String, Value>, config: &AppConfig) -> Result<String, String> {
    require_desktop_use(config)?;
    #[cfg(target_os = "macos")]
    {
        let x = need_f64(args, "x")?;
        let y = need_f64(args, "y")?;
        macos_input::mouse_move(x, y)?;
        Ok(format!("moved cursor to ({x}, {y})"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = args;
        Err("desktop_use_unsupported: mouse_move is currently macOS-only.".into())
    }
}

pub fn execute_mouse_click(
    args: &Map<String, Value>,
    config: &AppConfig,
) -> Result<String, String> {
    require_desktop_use(config)?;
    #[cfg(target_os = "macos")]
    {
        let x = need_f64(args, "x")?;
        let y = need_f64(args, "y")?;
        let button = parse_button(args)?;
        let clicks = args
            .get("clicks")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .clamp(1, 3) as u32;
        macos_input::mouse_click(x, y, button, clicks)?;
        Ok(format!("clicked at ({x}, {y}) ×{clicks}"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = args;
        Err("desktop_use_unsupported: mouse_click is currently macOS-only.".into())
    }
}

pub fn execute_mouse_drag(args: &Map<String, Value>, config: &AppConfig) -> Result<String, String> {
    require_desktop_use(config)?;
    #[cfg(target_os = "macos")]
    {
        let fx = need_f64(args, "from_x")?;
        let fy = need_f64(args, "from_y")?;
        let tx = need_f64(args, "to_x")?;
        let ty = need_f64(args, "to_y")?;
        let button = parse_button(args)?;
        macos_input::mouse_drag(fx, fy, tx, ty, button)?;
        Ok(format!("dragged ({fx}, {fy}) → ({tx}, {ty})"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = args;
        Err("desktop_use_unsupported: mouse_drag is currently macOS-only.".into())
    }
}

pub fn execute_scroll(args: &Map<String, Value>, config: &AppConfig) -> Result<String, String> {
    require_desktop_use(config)?;
    #[cfg(target_os = "macos")]
    {
        let dy = args
            .get("dy")
            .and_then(Value::as_i64)
            .ok_or("missing or non-integer 'dy'")? as i32;
        let dx = args.get("dx").and_then(Value::as_i64).unwrap_or(0) as i32;
        macos_input::scroll(dx, dy)?;
        Ok(format!("scrolled (dx={dx}, dy={dy})"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = args;
        Err("desktop_use_unsupported: scroll is currently macOS-only.".into())
    }
}

fn editor_event_path(args: &Map<String, Value>, cwd: &Path) -> Result<PathBuf, String> {
    let raw = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or("missing required string 'path'")?;
    let pb = PathBuf::from(raw);
    let resolved = if pb.is_absolute() { pb } else { cwd.join(&pb) };
    Ok(resolved)
}

fn emit_editor_event(name: &str, payload: Value) {
    let dir = vibn_events_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{name}.json"));
    let mut envelope = serde_json::Map::new();
    envelope.insert(
        "ts".into(),
        Value::String(chrono::Utc::now().to_rfc3339()),
    );
    envelope.insert("payload".into(), payload);
    let _ = std::fs::write(&path, serde_json::to_string(&envelope).unwrap_or_default());
}

pub fn execute_open_in_editor(args: &Map<String, Value>, cwd: &Path) -> Result<String, String> {
    let path = editor_event_path(args, cwd)?;
    let focus = args.get("focus").and_then(Value::as_bool).unwrap_or(true);
    let payload = serde_json::json!({
        "path": path.display().to_string(),
        "focus": focus,
    });
    emit_editor_event("open_in_editor", payload);
    Ok(format!("Requested to open {} in editor.", path.display()))
}

pub fn execute_show_in_explorer(args: &Map<String, Value>, cwd: &Path) -> Result<String, String> {
    let path = editor_event_path(args, cwd)?;
    let payload = serde_json::json!({ "path": path.display().to_string() });
    emit_editor_event("show_in_explorer", payload);
    Ok(format!("Requested to reveal {} in explorer.", path.display()))
}

pub fn run_hooks(
    config: &AppConfig,
    event: &str,
    context: Option<&Map<String, Value>>,
) -> Vec<HookResult> {
    let scripts = config
        .extra
        .get("hooks")
        .and_then(Value::as_object)
        .and_then(|hooks| hooks.get(event))
        .map(|value| match value {
            Value::String(script) => vec![script.clone()],
            Value::Array(items) => items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect(),
            _ => Vec::new(),
        })
        .unwrap_or_default();

    let mut results = Vec::new();
    for script in scripts {
        if let Some(result) = run_hook_script(&script, event, context) {
            results.push(result);
        }
    }
    results
}

fn run_hook_script(
    script: &str,
    event: &str,
    context: Option<&Map<String, Value>>,
) -> Option<HookResult> {
    let mut command = hook_shell_command(script);
    command.env("VIBN_EVENT", event);
    if let Some(context) = context {
        let context_value = Value::Object(context.clone());
        command.env("VIBN_CONTEXT", serde_json::to_string(&context_value).ok()?);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command.spawn().ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let output = child.wait_with_output().ok()?;
                return Some(HookResult {
                    script: script.to_owned(),
                    exit_code: output.status.code(),
                    stdout: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
                    stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
                });
            }
            Ok(None) if started.elapsed() >= HOOK_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(_) => return None,
        }
    }
}

fn hook_shell_command(script: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        command.args(["/C", script]);
        command
    }

    #[cfg(not(target_os = "windows"))]
    {
        let mut command = Command::new("sh");
        command.args(["-lc", script]);
        command
    }
}

fn execute_agent_tool(
    cache: &mut FileStateCache,
    name: &str,
    args: &Map<String, Value>,
    config: &AppConfig,
    cwd: &Path,
    confirm_callback: &mut Option<&mut impl FnMut(&str, &Map<String, Value>, &str) -> bool>,
    diff_callback: &mut Option<&mut impl FnMut(&Path, &str, &str) -> bool>,
) -> String {
    if name == "read_file" {
        let path = args.get("path").and_then(Value::as_str).unwrap_or("");
        let resolved = resolve_tool_path(cwd, path);
        if !args.contains_key("offset") && !args.contains_key("limit") {
            if let Some(cached) = cache.get(&resolved) {
                return cached;
            }
            let result = execute_tool_with_callbacks(
                name,
                args,
                config,
                cwd,
                confirm_callback,
                diff_callback,
            )
            .unwrap_or_else(|error| format!("[blocked: {error}]"));
            if !result.starts_with("Error:") && !result.starts_with("[blocked:") {
                cache.put(resolved, result.clone());
            }
            return result;
        }
    }

    if matches!(name, "write_file" | "edit_file" | "patch_file") {
        let path = args.get("path").and_then(Value::as_str).unwrap_or("");
        let resolved = resolve_tool_path(cwd, path);
        cache.invalidate(&resolved);
    }

    let result =
        execute_tool_with_callbacks(name, args, config, cwd, confirm_callback, diff_callback)
            .unwrap_or_else(|error| format!("[blocked: {error}]"));
    if matches!(name, "write_file" | "edit_file" | "patch_file") {
        let mut context = Map::new();
        context.insert("tool".to_owned(), Value::String(name.to_owned()));
        context.insert(
            "path".to_owned(),
            Value::String(
                args.get("path")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ),
        );
        run_hooks(config, HOOK_POST_EDIT, Some(&context));
    } else if name == "run_command" {
        let mut context = Map::new();
        context.insert(
            "command".to_owned(),
            Value::String(
                args.get("command")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ),
        );
        run_hooks(config, HOOK_POST_COMMAND, Some(&context));
    }
    result
}

fn maybe_auto_compact_messages(
    client: &OllamaClient,
    model: &str,
    messages: Vec<ChatMessage>,
    config: &AppConfig,
) -> (Vec<ChatMessage>, Option<usize>) {
    if !token_usage(model, &messages).needs_compact {
        return (messages, None);
    }
    if messages.len() <= 5 {
        return (messages, None);
    }

    let system = messages
        .first()
        .filter(|message| message.role == "system")
        .cloned();
    let recent_count = 6usize.min(messages.len());
    let split_at = messages.len().saturating_sub(recent_count);
    let recent = messages[split_at..].to_vec();
    let old = if system.is_some() {
        if split_at <= 1 {
            Vec::new()
        } else {
            messages[1..split_at].to_vec()
        }
    } else {
        messages[..split_at].to_vec()
    };
    if old.is_empty() {
        return (messages, None);
    }

    run_hooks(config, HOOK_PRE_COMPACT, None);
    let summary = summarize_auto_compacted_messages(client, model, &old)
        .unwrap_or_else(|| heuristic_compaction_summary(&old));
    let mut compacted = Vec::new();
    if let Some(system) = system {
        compacted.push(system);
    }
    compacted.push(ChatMessage::user(summary));
    compacted.push(ChatMessage::assistant("Got it."));
    compacted.extend(recent);
    run_hooks(config, HOOK_POST_COMPACT, None);
    (compacted, Some(old.len()))
}

fn summarize_auto_compacted_messages(
    client: &OllamaClient,
    model: &str,
    old: &[ChatMessage],
) -> Option<String> {
    let mut lines = Vec::new();
    for message in old {
        let content = message.content.trim();
        match message.role.as_str() {
            "user" => lines.push(format!("User: {}", truncate_for_summary(content, 400))),
            "assistant" => {
                if !message.tool_calls.is_empty() {
                    let names = message
                        .tool_calls
                        .iter()
                        .map(|call| call.function.name.as_str())
                        .collect::<Vec<_>>();
                    lines.push(format!("Agent called: {}", names.join(", ")));
                    if !content.is_empty() {
                        lines.push(format!(
                            "Agent said: {}",
                            truncate_for_summary(content, 200)
                        ));
                    }
                } else if !content.is_empty() {
                    lines.push(format!("Agent: {}", truncate_for_summary(content, 300)));
                }
            }
            "tool" => lines.push(format!(
                "Tool result: {}",
                truncate_for_summary(content.lines().next().unwrap_or(""), 120)
            )),
            _ => {}
        }
    }
    let conversation = lines
        .into_iter()
        .rev()
        .take(40)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    let prompt = format!(
        "Summarize this coding agent session in 5-8 bullet points. Focus on: what was built or changed, which files were modified, key decisions, and current state. Be specific and factual.\n\n{conversation}\n\nSummary:"
    );
    let summary = client.prompt(model, &prompt, None).ok()?;
    Some(format!("[Compacted session summary]\n{}", summary.trim()))
}

fn heuristic_compaction_summary(old: &[ChatMessage]) -> String {
    let mut parts = Vec::new();
    let mut index = 0usize;
    while index < old.len() {
        let message = &old[index];
        let content = message.content.trim();
        match message.role.as_str() {
            "user" => parts.push(format!("User: {}", truncate_for_summary(content, 150))),
            "assistant" => {
                if !message.tool_calls.is_empty() {
                    let names = message
                        .tool_calls
                        .iter()
                        .map(|call| call.function.name.as_str())
                        .collect::<Vec<_>>();
                    let mut results = Vec::new();
                    let mut next = index + 1;
                    while next < old.len() && old[next].role == "tool" {
                        results.push(truncate_for_summary(
                            old[next].content.lines().next().unwrap_or(""),
                            80,
                        ));
                        next += 1;
                    }
                    parts.push(format!(
                        "Agent used {} -> {}",
                        names.join(", "),
                        results.into_iter().take(3).collect::<Vec<_>>().join("; ")
                    ));
                    index = next;
                    continue;
                } else if !content.is_empty() {
                    parts.push(format!("Agent: {}", truncate_for_summary(content, 150)));
                }
            }
            "tool" => parts.push(format!("Tool: {}", truncate_for_summary(content, 80))),
            _ => {}
        }
        index += 1;
    }
    format!(
        "Previous conversation summary:\n{}",
        parts
            .into_iter()
            .rev()
            .take(20)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn truncate_for_summary(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

/// Streaming variant of [`run_agent_turns`]. Emits structured events as
/// the model produces text and as tools execute. Used by the JSON-event
/// CLI mode (`--json-events`) which forwards each event to AppLab over
/// stdout so the desktop chat panel can render the run live.
///
/// `emit` receives ready-to-serialize JSON values. Event shapes:
///   {"type":"step","step":N}
///   {"type":"text_delta","text":"..."}
///   {"type":"tool_start","id":"...","name":"...","input":{...}}
///   {"type":"tool_result","id":"...","name":"...","output":"...","isError":bool,"durationMs":N}
///   {"type":"done","stopped":"finished"|"max_steps"|"error","steps":N}
pub fn run_agent_turns_streaming<E>(
    client: &OllamaClient,
    model: &str,
    mut messages: Vec<ChatMessage>,
    config: &AppConfig,
    cwd: &Path,
    mut emit: E,
) -> Result<AgentRunResult, Box<dyn std::error::Error>>
where
    E: FnMut(serde_json::Value),
{
    let (compacted_messages, compacted_count) =
        maybe_auto_compact_messages(client, model, messages, config);
    messages = compacted_messages;
    let auto_compact_summary = if let Some(count) = compacted_count {
        let usage = token_usage(model, &messages);
        Some(format!(
            "Compacted {} messages. {} tokens ({}%)",
            count, usage.used, usage.percent
        ))
    } else {
        None
    };
    if let Some(user_message) = messages.iter().rev().find(|m| m.role == "user") {
        let mut context = Map::new();
        context.insert(
            "message".to_owned(),
            Value::String(user_message.content.clone()),
        );
        run_hooks(config, HOOK_PRE_CHAT, Some(&context));
    }

    let tools = tool_payloads_for_config(config);
    let mut file_cache = FileStateCache::default();
    let mut consecutive_errors = 0usize;
    let mut tool_rounds = 0usize;
    let mut no_confirm: Option<&mut ConfirmCallbackFn> = None;
    let mut no_diff: Option<&mut DiffCallbackFn> = None;
    let mut step_num = 0usize;
    let mut call_seq = 0u64;

    for _ in 0..MAX_AGENT_ROUNDS {
        step_num += 1;
        emit(json!({ "type": "step", "step": step_num }));

        let pending_messages = messages.clone();
        let mut assistant = client.chat_message_stream(
            model,
            pending_messages,
            Some(tools.clone()),
            |delta| {
                emit(json!({ "type": "text_delta", "text": delta }));
            },
        )?;

        if assistant.tool_calls.is_empty() && !assistant.content.trim().is_empty() {
            assistant.tool_calls = parse_tool_calls_from_text(&assistant.content);
        }
        let tool_calls = assistant.tool_calls.clone();
        let mut final_text = assistant.content.clone();
        messages.push(assistant);

        if tool_calls.is_empty() {
            let mut context = Map::new();
            context.insert("rounds".to_owned(), json!(tool_rounds));
            run_hooks(config, HOOK_POST_CHAT, Some(&context));
            emit(json!({
                "type": "done",
                "stopped": "finished",
                "steps": step_num,
            }));
            return Ok(AgentRunResult {
                messages,
                final_text,
                auto_compact_summary: auto_compact_summary.clone(),
            });
        }

        for tool_call in tool_calls {
            tool_rounds += 1;
            call_seq += 1;
            let tool_name = tool_call.function.name.clone();
            let call_id = format!("call_{call_seq}");
            emit(json!({
                "type": "tool_start",
                "id": call_id,
                "name": tool_name,
                "input": tool_call.function.arguments,
            }));
            let started = Instant::now();
            let result = execute_agent_tool(
                &mut file_cache,
                &tool_name,
                &tool_call.function.arguments,
                config,
                cwd,
                &mut no_confirm,
                &mut no_diff,
            );
            let duration_ms = started.elapsed().as_millis() as u64;
            let is_error =
                result.starts_with("Error:") || result.starts_with("[blocked:");
            emit(json!({
                "type": "tool_result",
                "id": call_id,
                "name": tool_name,
                "output": result,
                "isError": is_error,
                "durationMs": duration_ms,
            }));
            final_text = result.clone();
            messages.push(ChatMessage::tool(tool_name.clone(), result));

            if is_error {
                consecutive_errors += 1;
                if consecutive_errors >= 3 {
                    let mut context = Map::new();
                    context.insert("rounds".to_owned(), json!(tool_rounds));
                    run_hooks(config, HOOK_POST_CHAT, Some(&context));
                    emit(json!({
                        "type": "done",
                        "stopped": "error",
                        "steps": step_num,
                    }));
                    return Ok(AgentRunResult {
                        messages,
                        final_text,
                        auto_compact_summary: auto_compact_summary.clone(),
                    });
                }
            } else {
                consecutive_errors = 0;
            }
        }
    }

    let final_text = messages
        .last()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    emit(json!({
        "type": "done",
        "stopped": "max_steps",
        "steps": step_num,
    }));
    Ok(AgentRunResult {
        messages,
        final_text,
        auto_compact_summary,
    })
}

pub fn run_agent_turns(
    client: &OllamaClient,
    model: &str,
    messages: Vec<ChatMessage>,
    config: &AppConfig,
    cwd: &Path,
) -> Result<AgentRunResult, Box<dyn std::error::Error>> {
    let mut no_confirm: Option<&mut ConfirmCallbackFn> = None;
    let mut no_diff: Option<&mut DiffCallbackFn> = None;
    run_agent_turns_with_callbacks(
        client,
        model,
        messages,
        config,
        cwd,
        &mut no_confirm,
        &mut no_diff,
    )
}

pub fn run_agent_turns_with_callbacks<Confirm, Diff>(
    client: &OllamaClient,
    model: &str,
    mut messages: Vec<ChatMessage>,
    config: &AppConfig,
    cwd: &Path,
    confirm_callback: &mut Option<&mut Confirm>,
    diff_callback: &mut Option<&mut Diff>,
) -> Result<AgentRunResult, Box<dyn std::error::Error>>
where
    Confirm: FnMut(&str, &Map<String, Value>, &str) -> bool,
    Diff: FnMut(&Path, &str, &str) -> bool,
{
    let (compacted_messages, compacted_count) =
        maybe_auto_compact_messages(client, model, messages, config);
    messages = compacted_messages;
    let auto_compact_summary = if let Some(compacted_count) = compacted_count {
        let usage = token_usage(model, &messages);
        Some(format!(
            "Compacted {} messages. {} tokens ({}%)",
            compacted_count, usage.used, usage.percent
        ))
    } else {
        None
    };
    if let Some(user_message) = messages.iter().rev().find(|message| message.role == "user") {
        let mut context = Map::new();
        context.insert(
            "message".to_owned(),
            Value::String(user_message.content.clone()),
        );
        run_hooks(config, HOOK_PRE_CHAT, Some(&context));
    }
    let tools = tool_payloads_for_config(config);
    let mut consecutive_errors = 0usize;
    let mut file_cache = FileStateCache::default();
    let mut tool_rounds = 0usize;

    for _ in 0..MAX_AGENT_ROUNDS {
        let mut assistant = client.chat_message(model, messages.clone(), Some(tools.clone()))?;
        if assistant.tool_calls.is_empty() && !assistant.content.trim().is_empty() {
            assistant.tool_calls = parse_tool_calls_from_text(&assistant.content);
        }
        let mut final_text = assistant.content.clone();
        let tool_calls = assistant.tool_calls.clone();
        messages.push(assistant);

        if tool_calls.is_empty() {
            let mut context = Map::new();
            context.insert("rounds".to_owned(), json!(tool_rounds));
            run_hooks(config, HOOK_POST_CHAT, Some(&context));
            return Ok(AgentRunResult {
                messages,
                final_text,
                auto_compact_summary: auto_compact_summary.clone(),
            });
        }

        for tool_call in tool_calls {
            tool_rounds += 1;
            let tool_name = tool_call.function.name;
            let result = execute_agent_tool(
                &mut file_cache,
                &tool_name,
                &tool_call.function.arguments,
                config,
                cwd,
                confirm_callback,
                diff_callback,
            );
            final_text = result.clone();
            let mutating_computer_use =
                COMPUTER_USE_MUTATING_TOOLS.contains(&tool_name.as_str())
                    && !final_text.starts_with("Error:")
                    && !final_text.starts_with("desktop_use_");
            messages.push(ChatMessage::tool(tool_name.clone(), result));

            // Close the computer-use loop: after an action mutates the
            // screen, hand the next assistant turn a fresh screenshot. The
            // vision model produces a UI-focused description so non-vision
            // agents can still act on the new state; vision-capable agents
            // also get the raw image attached.
            if mutating_computer_use {
                if let Some(LoopScreenshot {
                    description,
                    image_b64,
                }) = capture_screenshot_for_loop(config)
                {
                    let content = format!(
                        "Screen state after {tool_name}:\n{description}\n\nUse this to decide the next action."
                    );
                    messages.push(ChatMessage::user_with_images(content, vec![image_b64]));
                }
            }

            if final_text.starts_with("Error:") || final_text.starts_with("[blocked:") {
                consecutive_errors += 1;
                if consecutive_errors >= 3 {
                    let mut context = Map::new();
                    context.insert("rounds".to_owned(), json!(tool_rounds));
                    run_hooks(config, HOOK_POST_CHAT, Some(&context));
                    return Ok(AgentRunResult {
                        messages,
                        final_text,
                        auto_compact_summary: auto_compact_summary.clone(),
                    });
                }
            } else {
                consecutive_errors = 0;
            }
        }

        if !messages
            .last()
            .map(|message| message.role == "tool")
            .unwrap_or(false)
        {
            return Ok(AgentRunResult {
                messages,
                final_text,
                auto_compact_summary: auto_compact_summary.clone(),
            });
        }
    }

    let final_text = messages
        .last()
        .map(|message| message.content.clone())
        .unwrap_or_default();
    Ok(AgentRunResult {
        messages,
        final_text,
        auto_compact_summary,
    })
}

impl OllamaClient {
    pub fn new(timeout: Duration) -> Result<Self, reqwest::Error> {
        let client = Client::builder().timeout(timeout).build()?;
        Ok(Self {
            base_url: ollama_base_url(),
            client,
        })
    }

    pub fn chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        Ok(self.chat_message(model, messages, None)?.content)
    }

    pub fn chat_message(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<Value>>,
    ) -> Result<ChatMessage, Box<dyn std::error::Error>> {
        retry_with_backoff(
            MAX_OLLAMA_RETRIES,
            OLLAMA_RETRY_BASE_DELAY,
            || -> Result<ChatMessage, Box<dyn std::error::Error>> {
                let response = self
                    .client
                    .post(format!("{}/api/chat", self.base_url))
                    .json(&ChatRequest {
                        model: model.to_owned(),
                        messages: messages.clone(),
                        stream: false,
                        tools: tools.clone(),
                    })
                    .send()?
                    .error_for_status()?;

                let payload: ChatResponse = response.json()?;
                Ok(payload.message)
            },
        )
    }

    /// Streaming variant of [`chat_message`]. Calls `on_delta` for every
    /// content fragment Ollama emits as it generates the reply, returning
    /// the fully-assembled final assistant message (including any
    /// `tool_calls`).
    ///
    /// Ollama's `/api/chat` with `stream: true` returns one NDJSON object
    /// per line. Each line carries an incremental `message` whose
    /// `content` is the next chunk. The terminal line has `done: true`
    /// and (for tool-capable models) the final `tool_calls` array.
    pub fn chat_message_stream<F>(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<Value>>,
        mut on_delta: F,
    ) -> Result<ChatMessage, Box<dyn std::error::Error>>
    where
        F: FnMut(&str),
    {
        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&ChatRequest {
                model: model.to_owned(),
                messages,
                stream: true,
                tools,
            })
            .send()?
            .error_for_status()?;

        let reader = BufReader::new(response);
        let mut accumulated_content = String::new();
        let mut accumulated_tool_calls: Vec<ToolCall> = Vec::new();
        let mut final_message: Option<ChatMessage> = None;

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let chunk: Value = match serde_json::from_str(&line) {
                Ok(value) => value,
                Err(_) => continue,
            };
            if let Some(message) = chunk.get("message") {
                if let Some(content) = message.get("content").and_then(Value::as_str) {
                    if !content.is_empty() {
                        on_delta(content);
                        accumulated_content.push_str(content);
                    }
                }
                if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
                    for call in calls {
                        if let Ok(parsed) = serde_json::from_value::<ToolCall>(call.clone()) {
                            accumulated_tool_calls.push(parsed);
                        }
                    }
                }
                if chunk
                    .get("done")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    // Last chunk — reconstruct the canonical message. We
                    // prefer the streamed fragments since some Ollama
                    // builds repeat or omit them in the terminal chunk.
                    let role = message
                        .get("role")
                        .and_then(Value::as_str)
                        .unwrap_or("assistant")
                        .to_owned();
                    final_message = Some(ChatMessage {
                        role,
                        content: accumulated_content.clone(),
                        tool_calls: accumulated_tool_calls.clone(),
                        name: None,
                        images: Vec::new(),
                    });
                }
            }
        }

        Ok(final_message.unwrap_or_else(|| ChatMessage {
            role: "assistant".to_owned(),
            content: accumulated_content,
            tool_calls: accumulated_tool_calls,
            name: None,
            images: Vec::new(),
        }))
    }

    pub fn prompt(
        &self,
        model: &str,
        prompt: &str,
        system_prompt: Option<&str>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut messages = Vec::new();
        if let Some(system_prompt) = system_prompt.filter(|value| !value.trim().is_empty()) {
            messages.push(ChatMessage::system(system_prompt));
        }
        messages.push(ChatMessage::user(prompt));
        self.chat(model, messages)
    }
}

fn retry_with_backoff<T, E, F>(
    max_retries: usize,
    base_delay: Duration,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
{
    let mut last_error = None;
    for attempt in 0..max_retries {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) => {
                last_error = Some(error);
                if attempt + 1 < max_retries {
                    let factor = 1u32 << attempt;
                    thread::sleep(base_delay.saturating_mul(factor));
                }
            }
        }
    }
    Err(last_error.expect("retry_with_backoff requires max_retries > 0"))
}

fn default_schema_version() -> u32 {
    1
}

fn default_model() -> String {
    "qwen2.5-coder:7b".to_owned()
}

fn vibn_config_dir() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".vibn")
}

pub fn vibn_config_file() -> PathBuf {
    vibn_config_dir().join("config.json")
}

pub fn vibn_transcripts_dir() -> PathBuf {
    vibn_config_dir().join("transcripts")
}

/// Directory where the agent drops small JSON files for the desktop UI to
/// react to (e.g. open_in_editor). Polled by the UI.
pub fn vibn_events_dir() -> PathBuf {
    vibn_config_dir().join("events")
}

pub fn vibn_observations_dir() -> PathBuf {
    vibn_config_dir().join("observations")
}

fn observations_projects_dir() -> PathBuf {
    vibn_observations_dir().join("projects")
}

fn global_observations_path() -> PathBuf {
    vibn_observations_dir().join("OBSERVATIONS.md")
}

fn project_slug(project_path: &Path) -> String {
    let name = project_path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("project");
    let digest = format!(
        "{:x}",
        md5::compute(project_path.to_string_lossy().as_bytes())
    );
    format!("{name}_{}", &digest[..6])
}

fn project_observations_path(project_path: &Path) -> PathBuf {
    observations_projects_dir()
        .join(project_slug(project_path))
        .join("OBSERVATIONS.md")
}

pub fn project_memory_entries(project_path: &Path) -> Result<Vec<ObservationEntry>, String> {
    let Some(content) = read_observation_file(&project_observations_path(project_path))? else {
        return Ok(Vec::new());
    };
    Ok(parse_observation_entries(&content))
}

pub fn remembered_facts_block(project_path: &Path) -> Result<Option<String>, String> {
    let entries = project_memory_entries(project_path)?;
    if entries.is_empty() {
        return Ok(None);
    }
    let mut lines = vec!["## Remembered facts".to_owned()];
    lines.extend(entries.into_iter().map(|entry| format!("- {}", entry.text)));
    Ok(Some(lines.join("\n")))
}

pub fn forget_project_memory(
    project_path: &Path,
    index: usize,
) -> Result<Option<ObservationEntry>, String> {
    if index == 0 {
        return Ok(None);
    }
    let path = project_observations_path(project_path);
    let Some(content) = read_observation_file(&path)? else {
        return Ok(None);
    };
    let mut entries = parse_observation_entries(&content);
    if index > entries.len() {
        return Ok(None);
    }
    let removed = entries.remove(index - 1);
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| "Observation path missing parent".to_owned())?,
    )
    .map_err(|error| format!("Error: {error}"))?;
    fs::write(&path, render_observation_entries(&entries))
        .map_err(|error| format!("Error: {error}"))?;
    Ok(Some(removed))
}

pub fn load_model_registry() -> Result<ModelRegistry, serde_json::Error> {
    serde_json::from_str(MODEL_REGISTRY_JSON)
}

pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        (text.len() / CHARS_PER_TOKEN).max(1)
    }
}

pub fn estimate_message_tokens(message: &ChatMessage) -> usize {
    let mut total = 4;
    total += estimate_tokens(&message.content);
    if !message.tool_calls.is_empty() {
        total += estimate_tokens(&serde_json::to_string(&message.tool_calls).unwrap_or_default());
    }
    total
}

pub fn estimate_conversation_tokens(messages: &[ChatMessage]) -> usize {
    messages.iter().map(estimate_message_tokens).sum()
}

pub fn get_context_window(model: &str) -> usize {
    match model {
        "qwen2.5-coder:7b" | "qwen2.5-coder:14b" | "qwen2.5-coder:32b" | "qwen2.5:7b"
        | "qwen2.5:14b" | "qwen2.5:32b" | "llama3.1:8b" | "llama3.1:70b" | "llama3.2:3b"
        | "llama3.3:70b" | "mistral-nemo:12b" => 131_072,
        "deepseek-coder-v2:16b" | "deepseek-r1:7b" | "deepseek-r1:14b" => 65_536,
        "mistral:7b" => 32_768,
        "codellama:7b" | "codellama:13b" | "codellama:34b" | "phi4:14b" => 16_384,
        "gemma2:9b" | "gemma2:27b" | "phi3:14b" => 8_192,
        _ => {
            let base = model.split(':').next().unwrap_or(model);
            if matches!(
                base,
                "qwen2.5-coder" | "qwen2.5" | "llama3.1" | "llama3.2" | "llama3.3" | "mistral-nemo"
            ) {
                131_072
            } else if matches!(base, "deepseek-coder-v2" | "deepseek-r1") {
                65_536
            } else if base == "mistral" {
                32_768
            } else if matches!(base, "codellama" | "phi4") {
                16_384
            } else if matches!(base, "gemma2" | "phi3") {
                8_192
            } else {
                DEFAULT_CONTEXT_WINDOW
            }
        }
    }
}

pub fn get_runtime_context_window(model: &str, context_window: usize) -> usize {
    let runtime = match model {
        "devstral:24b" | "gpt-oss:20b" | "qwen3-coder:30b" | "qwen2.5-coder:32b" => 8_192,
        _ => {
            let base = model.split(':').next().unwrap_or(model);
            if matches!(
                base,
                "devstral" | "gpt-oss" | "qwen3-coder" | "qwen2.5-coder"
            ) {
                8_192
            } else {
                DEFAULT_RUNTIME_CONTEXT_WINDOW
            }
        }
    };
    context_window.min(runtime)
}

pub fn token_usage(model: &str, messages: &[ChatMessage]) -> TokenUsage {
    let context_window = get_context_window(model);
    let runtime_context_window = get_runtime_context_window(model, context_window);
    let used = estimate_conversation_tokens(messages);
    let percent = if runtime_context_window == 0 {
        0.0
    } else {
        ((used as f64 / runtime_context_window as f64) * 1000.0).round() / 10.0
    };
    let remaining = runtime_context_window.saturating_sub(used);
    let ratio = if runtime_context_window == 0 {
        0.0
    } else {
        used as f64 / runtime_context_window as f64
    };
    TokenUsage {
        used,
        limit: runtime_context_window,
        percent,
        remaining,
        needs_warning: ratio >= 0.80,
        needs_compact: ratio >= 0.85,
        context_window,
        runtime_context_window,
    }
}

pub fn default_config_value() -> Result<Value, serde_json::Error> {
    serde_json::from_str(DEFAULT_CONFIG_JSON)
}

pub fn load_config() -> Result<AppConfig, Box<dyn std::error::Error>> {
    let mut merged = default_config_value()?;
    let path = vibn_config_file();
    if path.exists() {
        let user_value: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
        merge_json(&mut merged, user_value);
    }
    let mut config: AppConfig = serde_json::from_value(merged)?;
    if config.schema_version == 0 {
        config.schema_version = default_schema_version();
    }
    Ok(config)
}

pub fn save_config(config: &AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(vibn_config_dir())?;
    fs::write(vibn_config_file(), serde_json::to_string_pretty(config)?)?;
    Ok(())
}

pub fn get_ollama_models_path(config: &AppConfig) -> Option<PathBuf> {
    let trimmed = config.ollama_models_path.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(expand_home(trimmed))
    }
}

pub fn new_session_id() -> String {
    format!(
        "{}_{}",
        Local::now().format("%Y%m%d_%H%M%S"),
        &Uuid::new_v4().simple().to_string()[..6]
    )
}

pub fn save_transcript(
    session_id: &str,
    messages: &[ChatMessage],
    metadata: Option<Map<String, Value>>,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(vibn_transcripts_dir())?;
    let path = vibn_transcripts_dir().join(format!("{session_id}.jsonl"));
    let mut file = fs::File::create(path)?;
    let meta = TranscriptMetadata {
        entry_type: "metadata".to_owned(),
        session_id: session_id.to_owned(),
        timestamp: Local::now().to_rfc3339(),
        extra: metadata.unwrap_or_default(),
    };
    writeln!(file, "{}", serde_json::to_string(&meta)?)?;
    for message in messages {
        writeln!(file, "{}", serde_json::to_string(message)?)?;
    }
    Ok(())
}

pub fn append_to_transcript(
    session_id: &str,
    message: &ChatMessage,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(vibn_transcripts_dir())?;
    let path = vibn_transcripts_dir().join(format!("{session_id}.jsonl"));
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", serde_json::to_string(message)?)?;
    Ok(())
}

pub fn load_transcript(
    session_id: &str,
) -> Result<(Option<TranscriptMetadata>, Vec<ChatMessage>), Box<dyn std::error::Error>> {
    let path = vibn_transcripts_dir().join(format!("{session_id}.jsonl"));
    if !path.exists() {
        return Ok((None, Vec::new()));
    }

    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut metadata = None;
    let mut messages = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if value.get("type").and_then(Value::as_str) == Some("metadata") {
            metadata = serde_json::from_value(value).ok();
        } else if let Ok(message) = serde_json::from_value::<ChatMessage>(value) {
            messages.push(message);
        }
    }

    Ok((metadata, messages))
}

pub fn list_transcripts(limit: usize) -> Result<Vec<TranscriptEntry>, Box<dyn std::error::Error>> {
    let dir = vibn_transcripts_dir();
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut files: Vec<_> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("jsonl"))
        .collect();
    files.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));

    let mut entries = Vec::new();
    for entry in files.into_iter().take(limit) {
        let path = entry.path();
        let session_id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_owned();
        let file = fs::File::open(&path)?;
        let mut reader = BufReader::new(file);
        let mut first_line = String::new();
        let _ = reader.read_line(&mut first_line)?;
        let meta_value: Value =
            serde_json::from_str(first_line.trim()).unwrap_or(Value::Object(Map::new()));
        let message_count = reader.lines().count();
        entries.push(TranscriptEntry {
            session_id,
            timestamp: meta_value
                .get("timestamp")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            model: meta_value
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            project: meta_value
                .get("project")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            messages: message_count,
        });
    }

    Ok(entries)
}

fn execute_read_file(args: &Map<String, Value>, cwd: &Path) -> Result<String, String> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing required arg: path".to_owned())?;
    let path = resolve_tool_path(cwd, path);
    if !path.is_file() {
        return Err(format!("Error: not a file: {}", path.display()));
    }
    let content = fs::read_to_string(&path).map_err(|error| format!("Error: {error}"))?;
    let lines: Vec<_> = content.lines().collect();
    let total = lines.len();
    let offset = args
        .get("offset")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(1);
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map(|value| value as usize);
    let start = offset.saturating_sub(1);
    let end = limit
        .map(|limit| start.saturating_add(limit))
        .unwrap_or(total)
        .min(total);

    let (selected_start, selected_end, header) =
        if total > 1000 && !args.contains_key("offset") && !args.contains_key("limit") {
            (
                0,
                500.min(total),
                format!("[{total} lines, showing 1-500. Use offset/limit for more.]\n"),
            )
        } else {
            (start.min(total), end, String::new())
        };

    let body = lines[selected_start..selected_end]
        .iter()
        .enumerate()
        .map(|(index, line)| format!("{:>4}: {}", selected_start + index + 1, line))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(format!("{header}{body}").trim_end().to_owned())
}

fn execute_write_file<Diff>(
    args: &Map<String, Value>,
    cwd: &Path,
    diff_callback: &mut Option<&mut Diff>,
) -> Result<String, String>
where
    Diff: FnMut(&Path, &str, &str) -> bool,
{
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing required arg: path".to_owned())?;
    let content = args
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing required arg: content".to_owned())?;
    let path = resolve_tool_path(cwd, path);
    let old_content = fs::read_to_string(&path).unwrap_or_default();
    if let Some(callback) = diff_callback.as_deref_mut() {
        if !callback(&path, &old_content, content) {
            return Err("Permission denied: diff rejected".to_owned());
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("Error: {error}"))?;
    }
    fs::write(&path, content).map_err(|error| format!("Error: {error}"))?;
    Ok(format!(
        "Wrote {} ({} lines, {} bytes)",
        path.display(),
        line_count(content),
        content.len()
    ))
}

fn execute_edit_file<Diff>(
    args: &Map<String, Value>,
    cwd: &Path,
    diff_callback: &mut Option<&mut Diff>,
) -> Result<String, String>
where
    Diff: FnMut(&Path, &str, &str) -> bool,
{
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing required arg: path".to_owned())?;
    let old_string = args
        .get("old_string")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing required arg: old_string".to_owned())?;
    let new_string = args
        .get("new_string")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing required arg: new_string".to_owned())?;
    let path = resolve_tool_path(cwd, path);
    let content = fs::read_to_string(&path).map_err(|error| format!("Error: {error}"))?;
    let count = content.matches(old_string).count();
    if count == 0 {
        let hint = closest_line_hint(&content, old_string);
        return Err(format!("Error: string not found in file.{hint}"));
    }
    if count > 1 {
        return Err(format!(
            "Error: found {count} matches — provide more context to make it unique."
        ));
    }

    let new_content = content.replacen(old_string, new_string, 1);
    if let Some(callback) = diff_callback.as_deref_mut() {
        if !callback(&path, &content, &new_content) {
            return Err("Permission denied: diff rejected".to_owned());
        }
    }
    fs::write(&path, new_content).map_err(|error| format!("Error: {error}"))?;
    Ok(format!(
        "Edited {}\n{}",
        path.display(),
        render_replacement_diff(old_string, new_string)
    ))
}

fn execute_list_directory(args: &Map<String, Value>, cwd: &Path) -> Result<String, String> {
    let path_arg = args.get("path").and_then(Value::as_str).unwrap_or(".");
    let recursive = args
        .get("recursive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let depth = args.get("depth").and_then(Value::as_u64).unwrap_or(3) as usize;
    let path = resolve_tool_path(cwd, path_arg);
    if !path.is_dir() {
        return Err(format!("Error: not a directory: {}", path.display()));
    }

    if recursive {
        let mut lines = Vec::new();
        walk_directory(&path, &path, 0, depth, &mut lines)?;
        lines.truncate(200);
        Ok(lines.join("\n"))
    } else {
        let mut entries = fs::read_dir(&path)
            .map_err(|error| format!("Error: {error}"))?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        let mut lines = Vec::new();
        for entry in entries {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let entry_path = entry.path();
            if entry_path.is_dir() {
                lines.push(format!("  {file_name}/"));
            } else {
                let size = entry
                    .metadata()
                    .map(|metadata| metadata.len())
                    .unwrap_or_default();
                lines.push(format!("  {file_name}  ({})", human_size(size)));
            }
        }
        Ok(lines.join("\n"))
    }
}

fn execute_run_command(
    args: &Map<String, Value>,
    cwd: &Path,
    timeout_secs: u64,
) -> Result<String, String> {
    let command = args
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing required arg: command".to_owned())?;
    let command_cwd = args
        .get("working_dir")
        .and_then(Value::as_str)
        .map(|value| resolve_tool_path(cwd, value))
        .unwrap_or_else(|| cwd.to_path_buf());

    let mut child =
        shell_command(command, &command_cwd).map_err(|error| format!("Error: {error}"))?;
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let mut timed_out = false;

    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() >= deadline => {
                timed_out = true;
                child.kill().map_err(|error| format!("Error: {error}"))?;
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(error) => return Err(format!("Error: {error}")),
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("Error: {error}"))?;
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        if !combined.is_empty() && !combined.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str(&stderr);
    }
    if timed_out {
        if !combined.is_empty() && !combined.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str(&format!("[timed out after {timeout_secs}s]"));
    } else if !output.status.success() {
        if !combined.is_empty() && !combined.ends_with('\n') {
            combined.push('\n');
        }
        let code = output.status.code().unwrap_or(-1);
        combined.push_str(&format!("[exit code: {code}]"));
    }

    let mut lines = combined.lines().map(str::to_owned).collect::<Vec<_>>();
    if lines.len() > 200 {
        let omitted = lines.len() - 200;
        let mut shortened = lines[..100].to_vec();
        shortened.push(format!(""));
        shortened.push(format!("... [{omitted} lines omitted] ..."));
        shortened.push(format!(""));
        shortened.extend_from_slice(&lines[lines.len() - 100..]);
        lines = shortened;
    }
    let rendered = lines.join("\n");
    Ok(if rendered.trim().is_empty() {
        "[no output]".to_owned()
    } else {
        rendered.trim_end().to_owned()
    })
}

fn execute_search_code(args: &Map<String, Value>, cwd: &Path) -> Result<String, String> {
    let pattern = args
        .get("pattern")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing required arg: pattern".to_owned())?;
    let search_root =
        resolve_tool_path(cwd, args.get("path").and_then(Value::as_str).unwrap_or("."));
    if !search_root.is_dir() {
        return Err(format!("Error: not a directory: {}", search_root.display()));
    }
    let file_pattern = args.get("file_pattern").and_then(Value::as_str);

    if let Some(output) = try_ripgrep(pattern, &search_root, file_pattern) {
        return Ok(output);
    }

    fallback_search_code(pattern, &search_root, file_pattern)
}

fn execute_find_files(args: &Map<String, Value>, cwd: &Path) -> Result<String, String> {
    let pattern = args
        .get("pattern")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing required arg: pattern".to_owned())?;
    let search_root =
        resolve_tool_path(cwd, args.get("path").and_then(Value::as_str).unwrap_or("."));
    if !search_root.is_dir() {
        return Err(format!("Error: not a directory: {}", search_root.display()));
    }

    let matcher = compile_pattern(pattern)?;
    let mut matches = Vec::new();
    walk_files(&search_root, &search_root, &mut |_, relative| {
        let relative_text = normalize_relative_path(relative);
        if matcher.matches(&relative_text) {
            matches.push(relative_text);
        }
        matches.len() < 1000
    })?;

    if matches.is_empty() {
        return Ok(format!("No files matching '{pattern}'"));
    }

    matches.sort();
    let mut lines = matches.iter().take(100).cloned().collect::<Vec<_>>();
    if matches.len() > 100 {
        lines.push(format!("... +{} more", matches.len() - 100));
    }
    Ok(lines.join("\n"))
}

fn execute_patch_file<Diff>(
    args: &Map<String, Value>,
    cwd: &Path,
    diff_callback: &mut Option<&mut Diff>,
) -> Result<String, String>
where
    Diff: FnMut(&Path, &str, &str) -> bool,
{
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing required arg: path".to_owned())?;
    let edits = args
        .get("edits")
        .and_then(Value::as_array)
        .ok_or_else(|| "Missing required arg: edits".to_owned())?;
    let path = resolve_tool_path(cwd, path);
    let original_content = fs::read_to_string(&path).map_err(|error| format!("Error: {error}"))?;
    let mut content = original_content.clone();

    for (index, edit) in edits.iter().enumerate() {
        let Some(edit) = edit.as_object() else {
            return Err(format!("Error in edit {}: invalid edit object", index + 1));
        };
        let old = edit
            .get("old")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("Error in edit {}: missing old", index + 1))?;
        let new = edit
            .get("new")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("Error in edit {}: missing new", index + 1))?;
        let count = content.matches(old).count();
        if count == 0 {
            return Err(format!("Error in edit {}: string not found", index + 1));
        }
        if count > 1 {
            return Err(format!(
                "Error in edit {}: {} matches — need unique string",
                index + 1,
                count
            ));
        }
        content = content.replacen(old, new, 1);
    }

    if let Some(callback) = diff_callback.as_deref_mut() {
        if !callback(&path, &original_content, &content) {
            return Err("Permission denied: diff rejected".to_owned());
        }
    }
    fs::write(&path, content).map_err(|error| format!("Error: {error}"))?;
    Ok(format!(
        "Applied {} edits to {}",
        edits.len(),
        path.display()
    ))
}

fn execute_git(args: &Map<String, Value>, cwd: &Path) -> Result<String, String> {
    let git_args = args
        .get("args")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing required arg: args".to_owned())?;
    let mut child = shell_command(&format!("git {git_args}"), cwd)
        .map_err(|error| format!("Error: {error}"))?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut timed_out = false;

    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() >= deadline => {
                timed_out = true;
                child.kill().map_err(|error| format!("Error: {error}"))?;
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(error) => return Err(format!("Error: {error}")),
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("Error: {error}"))?;
    let rendered = render_process_output(
        output.stdout,
        output.stderr,
        output.status.code(),
        timed_out,
        30,
    );
    Ok(rendered)
}

fn execute_save_observation(args: &Map<String, Value>, cwd: &Path) -> Result<String, String> {
    let text = args
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing required arg: text".to_owned())?;
    let scope = args
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or("project");
    let path = if scope == "global" {
        global_observations_path()
    } else {
        project_observations_path(cwd)
    };

    ensure_observation_file(&path)?;
    trim_observations_if_needed(&path)?;

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("Error: {error}"))?;
    writeln!(
        file,
        "\n## {}\n{}",
        Local::now().format("%Y-%m-%d %H:%M"),
        text.trim()
    )
    .map_err(|error| format!("Error: {error}"))?;

    Ok(if scope == "global" {
        "Saved observation (global)".to_owned()
    } else {
        format!(
            "Saved observation ({})",
            path.parent()
                .and_then(|value| value.file_name())
                .and_then(|value| value.to_str())
                .unwrap_or("project")
        )
    })
}

fn execute_read_observations(cwd: &Path) -> Result<String, String> {
    let mut parts = Vec::new();

    if let Some(content) = read_observation_file(&global_observations_path())? {
        parts.push(content);
    }
    if let Some(content) = read_observation_file(&project_observations_path(cwd))? {
        parts.push(content);
    }

    Ok(if parts.is_empty() {
        "No observations yet.".to_owned()
    } else {
        parts.join("\n\n---\n\n")
    })
}

fn config_string(config: &AppConfig, key: &str) -> Option<String> {
    config
        .extra
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|s| !s.trim().is_empty())
}

fn config_u32(config: &AppConfig, key: &str) -> Option<u32> {
    config
        .extra
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
}

pub fn vision_model_for(config: &AppConfig) -> String {
    config_string(config, "vision_model").unwrap_or_else(|| "qwen2.5vl:7b".to_owned())
}

pub fn image_gen_model_for(config: &AppConfig) -> String {
    config_string(config, "image_gen_model").unwrap_or_else(|| "comfyui:sdxl-base".to_owned())
}

pub fn video_gen_model_for(config: &AppConfig) -> String {
    config_string(config, "video_gen_model").unwrap_or_else(|| "comfyui:ltx-video".to_owned())
}

pub fn comfyui_url_for(config: &AppConfig) -> String {
    let raw = config_string(config, "comfyui_url")
        .unwrap_or_else(|| "http://127.0.0.1:8188".to_owned());
    raw.trim_end_matches('/').to_owned()
}

fn comfyui_workflow_dir(config: &AppConfig) -> Option<PathBuf> {
    config_string(config, "comfyui_workflow_dir").map(|p| expand_home(Path::new(&p)))
}

fn encode_image_b64(path: &Path) -> Result<String, String> {
    use base64::Engine;
    let bytes = fs::read(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn vision_chat(model: &str, prompt: &str, images_b64: Vec<String>) -> Result<String, String> {
    let client = OllamaClient::new(Duration::from_secs(300))
        .map_err(|e| format!("ollama client: {e}"))?;
    let message = ChatMessage::user_with_images(prompt, images_b64);
    client.chat(model, vec![message]).map_err(|e| {
        let s = e.to_string();
        if s.contains("404") {
            format!(
                "Vision model `{model}` is not installed in Ollama. Pull it first: `ollama pull {model}`. Or pick another vision model in Settings → Models → Vision model (e.g. moondream:1.8b, qwen2.5vl:7b, llama3.2-vision:11b).",
            )
        } else {
            format!("vision call failed: {s}")
        }
    })
}

fn execute_read_image(
    args: &Map<String, Value>,
    config: &AppConfig,
    cwd: &Path,
) -> Result<String, String> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "read_image requires 'path'".to_owned())?;
    let resolved = resolve_tool_path(cwd, path);

    // Block reading images the agent generated itself — they live in
    // ~/.vibn/generated/, and the user already sees them rendered in the UI.
    let generated_root = vibn_generated_dir();
    if resolved.starts_with(&generated_root) {
        return Err(
            "Refusing to read_image on your own generated output. The user already sees the \
             image in the UI — produce a final response with the image path instead of \
             calling more tools."
                .to_owned(),
        );
    }

    let prompt = args
        .get("prompt")
        .and_then(Value::as_str)
        .unwrap_or("Describe this image in detail. Include any visible text.");
    let model = args
        .get("model")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| vision_model_for(config));
    let b64 = encode_image_b64(&resolved)?;
    vision_chat(&model, prompt, vec![b64])
}

fn execute_read_video(
    args: &Map<String, Value>,
    config: &AppConfig,
    cwd: &Path,
) -> Result<String, String> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "read_video requires 'path'".to_owned())?;
    let prompt = args
        .get("prompt")
        .and_then(Value::as_str)
        .unwrap_or("Summarize what happens across these video frames in order.");
    let frames = args
        .get("frames")
        .and_then(Value::as_u64)
        .map(|v| v.max(1).min(24) as u32)
        .or_else(|| config_u32(config, "video_frame_count"))
        .unwrap_or(6);
    let model = args
        .get("model")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| vision_model_for(config));
    let resolved = resolve_tool_path(cwd, path);

    let tmp_dir = env::temp_dir().join(format!("vibn-vid-{}", Uuid::new_v4()));
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("temp dir: {e}"))?;
    let pattern = tmp_dir.join("frame_%03d.jpg");

    let probe = Command::new("ffmpeg").arg("-version").output();
    if probe.is_err() {
        let _ = fs::remove_dir_all(&tmp_dir);
        return Err("ffmpeg not found on PATH (required for read_video)".to_owned());
    }

    let output = Command::new("ffmpeg")
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            &resolved.to_string_lossy(),
            "-vf",
            &format!("thumbnail,fps=1/max(1\\,({}/{})),scale=720:-2", frames, frames),
            "-frames:v",
            &frames.to_string(),
            &pattern.to_string_lossy(),
        ])
        .output()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    if !output.status.success() {
        let _ = fs::remove_dir_all(&tmp_dir);
        return Err(format!(
            "ffmpeg failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let mut images = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&tmp_dir)
        .map_err(|e| format!("temp dir read: {e}"))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("jpg"))
        .collect();
    entries.sort();
    for path in &entries {
        images.push(encode_image_b64(path)?);
    }
    let count = images.len();
    let result = vision_chat(&model, prompt, images);
    let _ = fs::remove_dir_all(&tmp_dir);
    match result {
        Ok(text) => Ok(format!("[analysed {count} frame(s)]\n{text}")),
        Err(e) => Err(e),
    }
}

fn comfy_spec_for(model_key: &str) -> Result<ComfyuiSpec, String> {
    let registry = load_model_registry().map_err(|e| format!("model registry: {e}"))?;
    let info = registry
        .get(model_key)
        .ok_or_else(|| format!("unknown model key '{model_key}'"))?;
    info.comfyui
        .clone()
        .ok_or_else(|| format!("model '{model_key}' is not a ComfyUI model"))
}

fn comfy_default_workflow(spec: &ComfyuiSpec, params: &ComfyParams) -> Value {
    let checkpoint_lower = spec.checkpoint.to_lowercase();
    let is_flux = checkpoint_lower.starts_with("flux") || checkpoint_lower.contains("flux1");
    if is_flux {
        comfy_flux_workflow(spec, params)
    } else {
        comfy_sd_workflow(spec, params)
    }
}

fn comfy_sd_workflow(spec: &ComfyuiSpec, p: &ComfyParams) -> Value {
    json!({
        "3": {"class_type":"KSampler","inputs":{
            "seed": p.seed, "steps": p.steps, "cfg": p.cfg,
            "sampler_name": p.sampler, "scheduler": p.scheduler,
            "denoise": 1.0,
            "model": ["4", 0], "positive": ["6", 0], "negative": ["7", 0],
            "latent_image": ["5", 0]
        }},
        "4": {"class_type":"CheckpointLoaderSimple","inputs":{"ckpt_name": spec.checkpoint}},
        "5": {"class_type":"EmptyLatentImage","inputs":{"width": p.width, "height": p.height, "batch_size": 1}},
        "6": {"class_type":"CLIPTextEncode","inputs":{"text": p.prompt, "clip": ["4", 1]}},
        "7": {"class_type":"CLIPTextEncode","inputs":{"text": p.negative, "clip": ["4", 1]}},
        "8": {"class_type":"VAEDecode","inputs":{"samples": ["3", 0], "vae": ["4", 2]}},
        "9": {"class_type":"SaveImage","inputs":{"images": ["8", 0], "filename_prefix": "vibn"}}
    })
}

fn comfy_flux_workflow(spec: &ComfyuiSpec, p: &ComfyParams) -> Value {
    json!({
        "3": {"class_type":"KSampler","inputs":{
            "seed": p.seed, "steps": p.steps, "cfg": p.cfg,
            "sampler_name": p.sampler, "scheduler": p.scheduler,
            "denoise": 1.0,
            "model": ["4", 0], "positive": ["6", 0], "negative": ["7", 0],
            "latent_image": ["5", 0]
        }},
        "4": {"class_type":"CheckpointLoaderSimple","inputs":{"ckpt_name": spec.checkpoint}},
        "5": {"class_type":"EmptyLatentImage","inputs":{"width": p.width, "height": p.height, "batch_size": 1}},
        "6": {"class_type":"CLIPTextEncode","inputs":{"text": p.prompt, "clip": ["4", 1]}},
        "7": {"class_type":"CLIPTextEncode","inputs":{"text": "", "clip": ["4", 1]}},
        "8": {"class_type":"VAEDecode","inputs":{"samples": ["3", 0], "vae": ["4", 2]}},
        "9": {"class_type":"SaveImage","inputs":{"images": ["8", 0], "filename_prefix": "vibn-flux"}}
    })
}

struct ComfyParams {
    prompt: String,
    negative: String,
    seed: u64,
    steps: u32,
    cfg: f64,
    width: u32,
    height: u32,
    sampler: String,
    scheduler: String,
}

fn build_comfy_params(
    args: &Map<String, Value>,
    spec: &ComfyuiSpec,
) -> ComfyParams {
    let prompt = args
        .get("prompt")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let negative = args
        .get("negative")
        .and_then(Value::as_str)
        .unwrap_or("blurry, low quality, text, watermark")
        .to_owned();
    let seed = args
        .get("seed")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            now ^ 0x9E37_79B9_7F4A_7C15
        });
    let steps = args
        .get("steps")
        .and_then(Value::as_u64)
        .map(|v| v as u32)
        .or(spec.default_steps)
        .unwrap_or(20);
    let cfg = args
        .get("cfg")
        .and_then(Value::as_f64)
        .or(spec.default_cfg)
        .unwrap_or(7.0);
    let width = args
        .get("width")
        .and_then(Value::as_u64)
        .map(|v| v as u32)
        .or(spec.default_width)
        .unwrap_or(1024);
    let height = args
        .get("height")
        .and_then(Value::as_u64)
        .map(|v| v as u32)
        .or(spec.default_height)
        .unwrap_or(1024);
    let sampler = args
        .get("sampler")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| spec.default_sampler.clone())
        .unwrap_or_else(|| "euler".to_owned());
    let scheduler = args
        .get("scheduler")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| spec.default_scheduler.clone())
        .unwrap_or_else(|| "normal".to_owned());
    ComfyParams { prompt, negative, seed, steps, cfg, width, height, sampler, scheduler }
}

fn render_workflow_template(template: &str, p: &ComfyParams, spec: &ComfyuiSpec, frames: u32) -> String {
    template
        .replace("{prompt}", &json_escape(&p.prompt))
        .replace("{negative}", &json_escape(&p.negative))
        .replace("{seed}", &p.seed.to_string())
        .replace("{steps}", &p.steps.to_string())
        .replace("{cfg}", &p.cfg.to_string())
        .replace("{width}", &p.width.to_string())
        .replace("{height}", &p.height.to_string())
        .replace("{frames}", &frames.to_string())
        .replace("{sampler}", &p.sampler)
        .replace("{scheduler}", &p.scheduler)
        .replace("{checkpoint}", &spec.checkpoint)
}

fn json_escape(s: &str) -> String {
    serde_json::to_string(s)
        .map(|q| q.trim_matches('"').to_owned())
        .unwrap_or_else(|_| s.to_owned())
}

fn load_workflow_template_for(model_key: &str, config: &AppConfig) -> Option<String> {
    let dir = comfyui_workflow_dir(config)?;
    let safe = model_key.replace(':', "_");
    let path = dir.join(format!("{safe}.json"));
    fs::read_to_string(path).ok()
}

pub fn comfyui_install_dir() -> PathBuf {
    vibn_config_dir().join("comfyui")
}

fn comfyui_pid_file() -> PathBuf {
    vibn_config_dir().join("comfyui.pid")
}

fn which_in_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let full = dir.join(name);
        if full.is_file() {
            return Some(full);
        }
    }
    None
}

fn comfyui_python(install_dir: &Path) -> Option<PathBuf> {
    let venv = install_dir.join("venv/bin/python");
    if venv.is_file() {
        return Some(venv);
    }
    which_in_path("python3").or_else(|| which_in_path("python"))
}

fn port_from_url(url: &str) -> u16 {
    url.trim_end_matches('/')
        .rsplit(':')
        .next()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(8188)
}

pub fn ping_comfyui(base_url: &str) -> bool {
    let client = match Client::builder()
        .timeout(Duration::from_millis(800))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    client
        .get(format!("{base_url}/system_stats"))
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

pub fn start_comfyui(config: &AppConfig) -> Result<(), String> {
    let install_dir = comfyui_install_dir();
    if !install_dir.join("main.py").is_file() {
        return Err(
            "No managed ComfyUI install at ~/.vibn/comfyui. Run /install-comfy first.".to_owned(),
        );
    }
    let url = comfyui_url_for(config);
    if ping_comfyui(&url) {
        return Ok(());
    }
    let python = comfyui_python(&install_dir)
        .ok_or_else(|| "python3 not found on PATH".to_owned())?;
    let port = port_from_url(&url);
    let log = fs::File::create(vibn_config_dir().join("comfyui.log"))
        .map_err(|e| format!("log file: {e}"))?;
    let err_log = log.try_clone().map_err(|e| format!("log clone: {e}"))?;
    let child = Command::new(python)
        .arg(install_dir.join("main.py"))
        .args(["--port", &port.to_string()])
        .current_dir(&install_dir)
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err_log))
        .spawn()
        .map_err(|e| format!("spawn comfyui: {e}"))?;
    let _ = fs::write(comfyui_pid_file(), child.id().to_string());
    Ok(())
}

pub fn stop_comfyui() -> Result<String, String> {
    let pid_path = comfyui_pid_file();
    let pid = fs::read_to_string(&pid_path)
        .map_err(|_| "No managed ComfyUI is running.".to_owned())?
        .trim()
        .to_owned();
    #[cfg(not(target_os = "windows"))]
    let output = Command::new("kill").arg(&pid).output();
    #[cfg(target_os = "windows")]
    let output = Command::new("taskkill").args(["/PID", &pid, "/F"]).output();
    let _ = fs::remove_file(&pid_path);
    output
        .map(|_| format!("Stopped ComfyUI (pid {pid})."))
        .map_err(|e| format!("kill failed: {e}"))
}

pub fn ensure_comfyui_running(config: &AppConfig) -> Result<(), String> {
    let url = comfyui_url_for(config);
    if ping_comfyui(&url) {
        return Ok(());
    }
    let install_dir = comfyui_install_dir();
    if !install_dir.join("main.py").is_file() {
        return Err(format!(
            "ComfyUI is not installed. To enable image generation, ask the user if they want to install it (~2GB download, 5-15 min), then call the `install_comfy` tool. After install, call `download_image_model` with a key like `comfyui:sdxl-base` or `comfyui:flux1-schnell`. ComfyUI URL is {url}."
        ));
    }
    start_comfyui(config)?;
    let deadline = Instant::now() + Duration::from_secs(90);
    while Instant::now() < deadline {
        if ping_comfyui(&url) {
            return Ok(());
        }
        thread::sleep(Duration::from_secs(2));
    }
    Err(format!(
        "ComfyUI was launched but didn't become ready at {url} within 90s. See ~/.vibn/comfyui.log"
    ))
}

pub fn install_comfyui<F: FnMut(&str)>(mut progress: F) -> Result<(), String> {
    let install_dir = comfyui_install_dir();
    if install_dir.join("main.py").is_file() {
        progress("ComfyUI already installed at ~/.vibn/comfyui");
        return Ok(());
    }
    let python =
        which_in_path("python3").ok_or_else(|| "python3 not found on PATH".to_owned())?;
    let git = which_in_path("git").ok_or_else(|| "git not found on PATH".to_owned())?;
    fs::create_dir_all(vibn_config_dir()).map_err(|e| format!("config dir: {e}"))?;

    progress(&format!("Cloning ComfyUI into {} ...", install_dir.display()));
    let status = Command::new(&git)
        .args(["clone", "--depth=1", "https://github.com/comfyanonymous/ComfyUI.git"])
        .arg(&install_dir)
        .status()
        .map_err(|e| format!("git: {e}"))?;
    if !status.success() {
        return Err("git clone failed".to_owned());
    }

    progress("Creating Python venv...");
    let status = Command::new(&python)
        .args(["-m", "venv", "venv"])
        .current_dir(&install_dir)
        .status()
        .map_err(|e| format!("venv: {e}"))?;
    if !status.success() {
        return Err("python -m venv failed".to_owned());
    }

    progress("Installing ComfyUI requirements (this can take 5-15 minutes)...");
    let pip = install_dir.join("venv/bin/pip");
    let status = Command::new(&pip)
        .args(["install", "--upgrade", "pip"])
        .current_dir(&install_dir)
        .status()
        .map_err(|e| format!("pip upgrade: {e}"))?;
    if !status.success() {
        return Err("pip upgrade failed".to_owned());
    }
    let status = Command::new(&pip)
        .args(["install", "-r", "requirements.txt"])
        .current_dir(&install_dir)
        .status()
        .map_err(|e| format!("pip install: {e}"))?;
    if !status.success() {
        return Err("pip install -r requirements.txt failed".to_owned());
    }

    fs::create_dir_all(install_dir.join("models/checkpoints"))
        .map_err(|e| format!("checkpoints dir: {e}"))?;
    progress("ComfyUI installed. Use /download-image-model <key> to fetch a checkpoint.");
    Ok(())
}

pub fn download_checkpoint_for<F: FnMut(&str)>(
    model_key: &str,
    mut progress: F,
) -> Result<PathBuf, String> {
    let spec = comfy_spec_for(model_key)?;
    let install_dir = comfyui_install_dir();
    if !install_dir.is_dir() {
        return Err("ComfyUI is not installed. Run /install-comfy first.".to_owned());
    }
    let ckpt_dir = install_dir.join("models/checkpoints");
    fs::create_dir_all(&ckpt_dir).map_err(|e| format!("checkpoints dir: {e}"))?;
    let target = ckpt_dir.join(&spec.checkpoint);
    if target.is_file() {
        progress(&format!("Checkpoint already present: {}", target.display()));
        return Ok(target);
    }
    let url = spec
        .download_url
        .as_ref()
        .ok_or_else(|| format!("model '{model_key}' has no download_url"))?;
    progress(&format!("Downloading {url} ..."));
    let client = Client::builder()
        .timeout(Duration::from_secs(60 * 60 * 4))
        .build()
        .map_err(|e| format!("http: {e}"))?;
    let mut resp = client
        .get(url)
        .send()
        .map_err(|e| format!("download: {e}"))?
        .error_for_status()
        .map_err(|e| format!("download status: {e}"))?;
    let mut file = fs::File::create(&target).map_err(|e| format!("create: {e}"))?;
    std::io::copy(&mut resp, &mut file).map_err(|e| format!("copy: {e}"))?;
    progress(&format!("Saved {}", target.display()));
    Ok(target)
}

fn execute_install_comfy() -> Result<String, String> {
    Err(
        "REQUIRES USER CONFIRMATION. Do not call this tool. Tell the user that installing \
         ComfyUI is a multi-GB download and ask them to confirm — they will click 'Install \
         ComfyUI' in the desktop, or run /install-comfy in the CLI. Only those user-initiated \
         paths will actually install."
            .to_owned(),
    )
}

fn execute_download_image_model(args: &Map<String, Value>) -> Result<String, String> {
    let model_key = args
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("comfyui:sdxl-base");
    Err(format!(
        "REQUIRES USER CONFIRMATION. Do not call this tool. Tell the user that downloading \
         `{model_key}` is a multi-GB download and ask them to confirm — they will click \
         'Download' in the desktop, or run /download-image-model {model_key} in the CLI. \
         Only those user-initiated paths will actually download.",
    ))
}

fn comfyui_submit_and_fetch(
    config: &AppConfig,
    workflow: Value,
    out_dir: &Path,
    file_prefix: &str,
) -> Result<Vec<PathBuf>, String> {
    let base = comfyui_url_for(config);
    let client = Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let client_id = Uuid::new_v4().to_string();
    let submit_body = json!({"prompt": workflow, "client_id": client_id});

    let submit: Value = client
        .post(format!("{base}/prompt"))
        .json(&submit_body)
        .send()
        .map_err(|e| format!("ComfyUI submit (is it running at {base}?): {e}"))?
        .error_for_status()
        .map_err(|e| format!("ComfyUI rejected prompt: {e}"))?
        .json()
        .map_err(|e| format!("ComfyUI submit json: {e}"))?;
    let prompt_id = submit
        .get("prompt_id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("ComfyUI did not return prompt_id: {submit}"))?
        .to_owned();

    let deadline = Instant::now() + Duration::from_secs(600);
    let mut history: Value;
    loop {
        if Instant::now() > deadline {
            return Err(format!("ComfyUI generation timed out for prompt {prompt_id}"));
        }
        thread::sleep(Duration::from_millis(800));
        let resp = client
            .get(format!("{base}/history/{prompt_id}"))
            .send()
            .map_err(|e| format!("ComfyUI poll: {e}"))?;
        if !resp.status().is_success() {
            continue;
        }
        history = resp
            .json()
            .map_err(|e| format!("ComfyUI poll json: {e}"))?;
        if history.get(&prompt_id).is_some() {
            break;
        }
    }

    fs::create_dir_all(out_dir).map_err(|e| format!("output dir: {e}"))?;
    let mut saved = Vec::new();
    let outputs = history
        .get_mut(&prompt_id)
        .and_then(|v| v.get_mut("outputs"))
        .ok_or_else(|| "ComfyUI returned no outputs".to_owned())?
        .clone();
    let nodes = outputs
        .as_object()
        .ok_or_else(|| "ComfyUI outputs not an object".to_owned())?;

    for (_node_id, node_out) in nodes {
        for key in ["images", "gifs", "videos"] {
            if let Some(items) = node_out.get(key).and_then(Value::as_array) {
                for item in items {
                    let filename = item.get("filename").and_then(Value::as_str).unwrap_or("");
                    let subfolder = item.get("subfolder").and_then(Value::as_str).unwrap_or("");
                    let typ = item.get("type").and_then(Value::as_str).unwrap_or("output");
                    if filename.is_empty() {
                        continue;
                    }
                    let url = format!(
                        "{base}/view?filename={}&subfolder={}&type={}",
                        urlencode(filename),
                        urlencode(subfolder),
                        urlencode(typ)
                    );
                    let bytes = client
                        .get(&url)
                        .send()
                        .and_then(|r| r.error_for_status())
                        .and_then(|r| r.bytes())
                        .map_err(|e| format!("ComfyUI fetch {filename}: {e}"))?;
                    let ext = Path::new(filename)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("png");
                    let target = out_dir.join(format!(
                        "{file_prefix}-{}.{ext}",
                        &Uuid::new_v4().to_string()[..8]
                    ));
                    fs::write(&target, &bytes).map_err(|e| format!("write output: {e}"))?;
                    saved.push(target);
                }
            }
        }
    }

    if saved.is_empty() {
        return Err("ComfyUI completed but produced no image/video outputs".to_owned());
    }
    Ok(saved)
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn vibn_generated_dir() -> PathBuf {
    vibn_config_dir().join("generated")
}

fn execute_generate_image(
    args: &Map<String, Value>,
    config: &AppConfig,
    cwd: &Path,
) -> Result<String, String> {
    let model_key = args
        .get("model")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| image_gen_model_for(config));
    let spec = comfy_spec_for(&model_key)?;
    if spec.kind != "image" {
        return Err(format!(
            "model '{model_key}' is kind '{}', not 'image'",
            spec.kind
        ));
    }
    let params = build_comfy_params(args, &spec);

    let workflow: Value = match args
        .get("workflow_template")
        .and_then(Value::as_str)
        .map(|s| s.to_owned())
        .or_else(|| load_workflow_template_for(&model_key, config))
    {
        Some(template) => {
            let rendered = render_workflow_template(&template, &params, &spec, 1);
            serde_json::from_str(&rendered)
                .map_err(|e| format!("workflow template JSON error: {e}"))?
        }
        None => comfy_default_workflow(&spec, &params),
    };

    let out_dir = args
        .get("output_path")
        .and_then(Value::as_str)
        .map(|p| resolve_tool_path(cwd, p))
        .unwrap_or_else(vibn_generated_dir);
    ensure_comfyui_running(config)?;
    let install_dir = comfyui_install_dir();
    if install_dir.is_dir()
        && !install_dir
            .join("models/checkpoints")
            .join(&spec.checkpoint)
            .is_file()
    {
        let size_gb = load_model_registry()
            .ok()
            .and_then(|r| r.get(&model_key).map(|m| m.size_gb))
            .unwrap_or(0.0);
        return Err(format!(
            "Checkpoint `{}` is not present in the managed install. Ask the user if they want to download it (~{:.1} GB), then call the `download_image_model` tool with model=\"{}\".",
            spec.checkpoint, size_gb, model_key
        ));
    }
    let saved = comfyui_submit_and_fetch(config, workflow, &out_dir, "image")?;
    Ok(format!(
        "SUCCESS — generated {} image(s) using {model_key}. STOP HERE: reply to the user \
         with the image path(s) below and a short confirmation. Do NOT call read_image on \
         your own output, do NOT generate another image unless the user explicitly asks for \
         a variation, do NOT call any further tools for this request.\n{}",
        saved.len(),
        saved
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

fn execute_generate_video(
    args: &Map<String, Value>,
    config: &AppConfig,
    cwd: &Path,
) -> Result<String, String> {
    let model_key = args
        .get("model")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| video_gen_model_for(config));
    let spec = comfy_spec_for(&model_key)?;
    if spec.kind != "video" {
        return Err(format!(
            "model '{model_key}' is kind '{}', not 'video'",
            spec.kind
        ));
    }
    let params = build_comfy_params(args, &spec);
    let frames = args
        .get("frames")
        .and_then(Value::as_u64)
        .map(|v| v as u32)
        .unwrap_or(25);

    let template = args
        .get("workflow_template")
        .and_then(Value::as_str)
        .map(|s| s.to_owned())
        .or_else(|| load_workflow_template_for(&model_key, config))
        .ok_or_else(|| format!(
            "Video model '{model_key}' needs a ComfyUI workflow template. Pass `workflow_template` or save one at <comfyui_workflow_dir>/{}.json",
            model_key.replace(':', "_")
        ))?;
    let rendered = render_workflow_template(&template, &params, &spec, frames);
    let workflow: Value = serde_json::from_str(&rendered)
        .map_err(|e| format!("workflow template JSON error: {e}"))?;

    let out_dir = args
        .get("output_path")
        .and_then(Value::as_str)
        .map(|p| resolve_tool_path(cwd, p))
        .unwrap_or_else(vibn_generated_dir);
    ensure_comfyui_running(config)?;
    let saved = comfyui_submit_and_fetch(config, workflow, &out_dir, "video")?;
    Ok(format!(
        "Generated {} video file(s) using {model_key}:\n{}",
        saved.len(),
        saved
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

pub fn build_system_profile(storage_path: impl AsRef<Path>) -> SystemProfile {
    let storage_path = expand_home(storage_path.as_ref());
    let total_ram_gb = detect_total_ram_bytes().map(bytes_to_gb);
    let storage_free_gb = get_storage_free_gb(&storage_path);
    SystemProfile {
        system: normalize_system_name(env::consts::OS),
        machine: env::consts::ARCH.to_owned(),
        cpu_count: thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(0),
        total_ram_gb,
        storage_path,
        storage_free_gb,
    }
}

pub fn model_fit(info: &ModelInfo, profile: &SystemProfile) -> ModelFit {
    let reserve_gb = (info.size_gb * 0.15).clamp(2.0, 8.0);
    if let Some(free_gb) = profile.storage_free_gb {
        if free_gb < info.size_gb + reserve_gb {
            return ModelFit::TightDisk;
        }
    }

    let Some(total_ram_gb) = profile.total_ram_gb else {
        return ModelFit::Unknown;
    };

    if total_ram_gb >= f64::from(info.recommended_ram_gb) {
        ModelFit::Good
    } else if total_ram_gb >= f64::from(info.min_ram_gb) {
        ModelFit::Tight
    } else {
        ModelFit::TooLarge
    }
}

pub fn format_gb(value: Option<f64>) -> String {
    let Some(value) = value else {
        return "unknown".to_owned();
    };
    if value >= 100.0 {
        format!("{value:.0}GB")
    } else if value >= 10.0 {
        format!("{value:.1}GB")
    } else {
        format!("{value:.2}GB")
    }
}

pub fn format_system_summary(profile: &SystemProfile) -> String {
    let ram = match profile.total_ram_gb {
        Some(ram) => format!("{ram:.1}GB RAM"),
        None => "RAM unknown".to_owned(),
    };
    let disk = match profile.storage_free_gb {
        Some(free) => format!("{free:.1}GB free"),
        None => "disk unknown".to_owned(),
    };
    format!(
        "{} {} · {} CPU · {} · {} @ {}",
        profile.system,
        profile.machine,
        profile.cpu_count,
        ram,
        disk,
        profile.storage_path.display()
    )
}

fn expand_home(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let text = path.to_string_lossy();
    if text == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(text.as_ref()));
    }
    if let Some(rest) = text.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    path.to_path_buf()
}

fn resolve_tool_path(cwd: &Path, path: &str) -> PathBuf {
    let expanded = expand_home(path);
    if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    }
}

fn ensure_observation_file(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("Error: {error}"))?;
    }
    if !path.exists() {
        fs::write(path, OBSERVATIONS_HEADER).map_err(|error| format!("Error: {error}"))?;
    }
    Ok(())
}

fn trim_observations_if_needed(path: &Path) -> Result<(), String> {
    let size = fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    if size <= MAX_OBSERVATION_FILE_SIZE {
        return Ok(());
    }

    let content = fs::read_to_string(path).map_err(|error| format!("Error: {error}"))?;
    let entries = content.split("\n## ").collect::<Vec<_>>();
    if entries.len() <= 2 {
        return Ok(());
    }

    let header = entries[0];
    let keep = &entries[entries.len() / 2..];
    let trimmed = format!("{header}\n## {}", keep.join("\n## "));
    fs::write(path, trimmed).map_err(|error| format!("Error: {error}"))?;
    Ok(())
}

fn read_observation_file(path: &Path) -> Result<Option<String>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let content = fs::read_to_string(path).map_err(|error| format!("Error: {error}"))?;
    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed == "# Observations" {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_owned()))
    }
}

fn parse_observation_entries(content: &str) -> Vec<ObservationEntry> {
    let mut entries = Vec::new();
    let mut heading: Option<String> = None;
    let mut body = Vec::new();

    for line in content.lines() {
        if line == "# Observations" {
            continue;
        }
        if let Some(rest) = line.strip_prefix("## ") {
            if let Some(current_heading) = heading.take() {
                let text = body.join("\n").trim().to_owned();
                if !text.is_empty() {
                    entries.push(ObservationEntry {
                        heading: current_heading,
                        text,
                    });
                }
                body.clear();
            }
            heading = Some(rest.trim().to_owned());
        } else if heading.is_some() {
            body.push(line.to_owned());
        }
    }

    if let Some(current_heading) = heading {
        let text = body.join("\n").trim().to_owned();
        if !text.is_empty() {
            entries.push(ObservationEntry {
                heading: current_heading,
                text,
            });
        }
    }

    entries
}

fn render_observation_entries(entries: &[ObservationEntry]) -> String {
    if entries.is_empty() {
        return OBSERVATIONS_HEADER.to_owned();
    }
    let mut rendered = String::from(OBSERVATIONS_HEADER);
    rendered.push_str(
        &entries
            .iter()
            .map(|entry| format!("## {}\n{}", entry.heading, entry.text))
            .collect::<Vec<_>>()
            .join("\n\n"),
    );
    rendered.push('\n');
    rendered
}

fn try_ripgrep(pattern: &str, root: &Path, file_pattern: Option<&str>) -> Option<String> {
    let mut command = Command::new("rg");
    command.args(["--no-heading", "--line-number", "--max-count", "50", "-i"]);
    if let Some(file_pattern) = file_pattern {
        command.args(["--glob", file_pattern]);
    }
    command.arg(pattern).arg(".");
    command.current_dir(root);
    let output = command.output().ok()?;
    if !output.status.success() && output.status.code().unwrap_or_default() > 1 {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Some(if stdout.is_empty() {
        format!("No matches for '{pattern}'")
    } else {
        stdout
    })
}

fn fallback_search_code(
    pattern: &str,
    root: &Path,
    file_pattern: Option<&str>,
) -> Result<String, String> {
    let matcher = match file_pattern {
        Some(file_pattern) => Some(compile_pattern(file_pattern)?),
        None => None,
    };
    let pattern_lower = pattern.to_lowercase();
    let mut matches = Vec::new();

    walk_files(root, root, &mut |path, relative| {
        let relative_text = normalize_relative_path(relative);
        if let Some(matcher) = &matcher {
            if !matcher.matches(&relative_text) {
                return true;
            }
        }

        let Ok(content) = fs::read_to_string(path) else {
            return true;
        };

        for (index, line) in content.lines().enumerate() {
            if line.to_lowercase().contains(&pattern_lower) {
                matches.push(format!(
                    "{relative_text}:{}: {}",
                    index + 1,
                    line.trim_end()
                ));
                if matches.len() >= 50 {
                    return false;
                }
            }
        }

        true
    })?;

    if matches.is_empty() {
        Ok(format!("No matches for '{pattern}'"))
    } else {
        let mut output = matches.join("\n");
        if matches.len() >= 50 {
            output.push_str("\n[truncated at 50]");
        }
        Ok(output)
    }
}

fn walk_directory(
    root: &Path,
    current: &Path,
    depth: usize,
    max_depth: usize,
    lines: &mut Vec<String>,
) -> Result<(), String> {
    if depth > max_depth {
        return Ok(());
    }

    let name = if depth == 0 {
        root.file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.display().to_string())
    } else {
        current
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| current.display().to_string())
    };
    let indent = "  ".repeat(depth);
    lines.push(format!("{indent}{name}/"));

    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut entries = fs::read_dir(current)
        .map_err(|error| format!("Error: {error}"))?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if path.is_dir() {
            if !ignored_directory(&file_name) {
                directories.push(path);
            }
        } else if files.len() < 30 {
            files.push(file_name);
        }
    }

    for file in &files {
        lines.push(format!("{indent}  {file}"));
    }
    if files.len() == 30 {
        let total_files = fs::read_dir(current)
            .map_err(|error| format!("Error: {error}"))?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_file())
            .count();
        if total_files > 30 {
            lines.push(format!("{indent}  ... +{} more", total_files - 30));
        }
    }

    for directory in directories {
        walk_directory(root, &directory, depth + 1, max_depth, lines)?;
    }
    Ok(())
}

fn walk_files<F>(root: &Path, current: &Path, visit: &mut F) -> Result<(), String>
where
    F: FnMut(&Path, &Path) -> bool,
{
    let mut entries = fs::read_dir(current)
        .map_err(|error| format!("Error: {error}"))?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if ignored_directory(&file_name) {
                continue;
            }
            walk_files(root, &path, visit)?;
        } else {
            if ignored_extension(&path) {
                continue;
            }
            let relative = path.strip_prefix(root).unwrap_or(&path);
            if !visit(&path, relative) {
                return Ok(());
            }
        }
    }

    Ok(())
}

fn ignored_directory(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "node_modules"
            | "__pycache__"
            | "venv"
            | ".venv"
            | ".next"
            | "dist"
            | "build"
            | "target"
            | ".cache"
            | ".turbo"
    )
}

fn ignored_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "pyc" | "pyo" | "o" | "so" | "jpg" | "png" | "gif" | "zip" | "tar" | "gz" | "lock"
    )
}

fn line_count(content: &str) -> usize {
    content.matches('\n').count() + usize::from(!content.is_empty() && !content.ends_with('\n'))
}

fn closest_line_hint(content: &str, target: &str) -> String {
    let needle = target.lines().next().unwrap_or("").trim();
    if needle.is_empty() {
        return String::new();
    }

    let mut best_line = None;
    let mut best_score = 0usize;
    for line in content.lines() {
        let score = shared_prefix_len(line.trim(), needle);
        if score > best_score {
            best_score = score;
            best_line = Some(line);
        }
    }

    match best_line.filter(|_| best_score > 0) {
        Some(line) => format!(" Similar line: '{line}'"),
        None => String::new(),
    }
}

fn shared_prefix_len(left: &str, right: &str) -> usize {
    left.chars()
        .zip(right.chars())
        .take_while(|(left, right)| left == right)
        .count()
}

fn human_size(size: u64) -> String {
    let mut value = size as f64;
    for unit in ["B", "KB", "MB", "GB"] {
        if value < 1024.0 {
            return if unit == "B" {
                format!("{value:.0}{unit}")
            } else {
                format!("{value:.1}{unit}")
            };
        }
        value /= 1024.0;
    }
    format!("{value:.1}TB")
}

fn shell_command(command: &str, cwd: &Path) -> Result<std::process::Child, std::io::Error> {
    #[cfg(target_os = "windows")]
    {
        return Command::new("cmd")
            .args(["/C", command])
            .current_dir(cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("sh")
            .args(["-lc", command])
            .current_dir(cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
    }
}

fn compile_pattern(pattern: &str) -> Result<Pattern, String> {
    Pattern::new(pattern)
        .map_err(|error| format!("Error: invalid glob pattern '{pattern}': {error}"))
}

fn normalize_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn render_replacement_diff(old: &str, new: &str) -> String {
    let mut lines = vec!["--- old".to_owned(), "+++ new".to_owned()];
    lines.extend(old.lines().map(|line| format!("-{line}")));
    lines.extend(new.lines().map(|line| format!("+{line}")));
    lines.into_iter().take(20).collect::<Vec<_>>().join("\n")
}

fn render_process_output(
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_code: Option<i32>,
    timed_out: bool,
    timeout_secs: u64,
) -> String {
    let mut combined = String::from_utf8_lossy(&stdout).to_string();
    let stderr = String::from_utf8_lossy(&stderr);
    if !stderr.trim().is_empty() {
        if !combined.is_empty() && !combined.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str(&stderr);
    }
    if timed_out {
        if !combined.is_empty() && !combined.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str(&format!("[timed out after {timeout_secs}s]"));
    } else if exit_code.unwrap_or_default() != 0 && combined.trim().is_empty() {
        combined.push_str(&format!("[exit code: {}]", exit_code.unwrap_or(-1)));
    }

    let mut lines = combined.lines().map(str::to_owned).collect::<Vec<_>>();
    if lines.len() > 200 {
        let omitted = lines.len() - 200;
        let mut shortened = lines[..100].to_vec();
        shortened.push(String::new());
        shortened.push(format!("... [{omitted} lines omitted] ..."));
        shortened.push(String::new());
        shortened.extend_from_slice(&lines[lines.len() - 100..]);
        lines = shortened;
    }

    let rendered = lines.join("\n");
    if rendered.trim().is_empty() {
        "[no output]".to_owned()
    } else {
        rendered.trim_end().to_owned()
    }
}

fn check_tool_permission<Confirm>(
    name: &str,
    args: &Map<String, Value>,
    config: &AppConfig,
    confirm_callback: &mut Option<&mut Confirm>,
) -> Result<(), String>
where
    Confirm: FnMut(&str, &Map<String, Value>, &str) -> bool,
{
    let permissions = config.extra.get("permissions").and_then(Value::as_object);

    if let Some(permissions) = permissions {
        for rule in permission_rules(permissions, "always_allow") {
            if rule_matches(rule, name, args) {
                return Ok(());
            }
        }

        for rule in permission_rules(permissions, "always_deny") {
            if rule_matches(rule, name, args) {
                return Err(format!(
                    "Permission denied: denied by rule: {}",
                    rule.get("reason")
                        .and_then(Value::as_str)
                        .unwrap_or("policy")
                ));
            }
        }

        for rule in permission_rules(permissions, "confirm") {
            if rule_matches(rule, name, args) {
                let reason = rule
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("requires confirmation");
                if let Some(callback) = confirm_callback.as_deref_mut() {
                    if callback(name, args, reason) {
                        return Ok(());
                    }
                }
                return Err(format!("Permission denied: needs confirmation: {reason}"));
            }
        }
    }

    if name == "run_command" {
        let command = args
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase();
        for pattern in [
            "rm -rf",
            "rm -r",
            "rmdir",
            "git reset --hard",
            "git push --force",
            "git push -f",
            "git clean -f",
            "git checkout .",
            "drop table",
            "drop database",
            "truncate",
            "dd if=",
            "mkfs",
            "format",
            "> /dev/",
            "chmod -r 777",
        ] {
            if command.contains(pattern) {
                if let Some(callback) = confirm_callback.as_deref_mut() {
                    if callback(name, args, &format!("destructive command: {pattern}")) {
                        return Ok(());
                    }
                }
                return Err(format!("Permission denied: destructive command: {pattern}"));
            }
        }
    }

    if name == "git" {
        let command = args
            .get("args")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase();
        for pattern in [
            "reset --hard",
            "push --force",
            "push -f",
            "clean -f",
            "checkout .",
        ] {
            if command.contains(pattern) {
                let reason = format!("destructive command: git {pattern}");
                if let Some(callback) = confirm_callback.as_deref_mut() {
                    if callback(name, args, &reason) {
                        return Ok(());
                    }
                }
                return Err(format!("Permission denied: {reason}"));
            }
        }
    }

    if matches!(name, "write_file" | "edit_file" | "patch_file") {
        let path = args.get("path").and_then(Value::as_str).unwrap_or("");
        let file_name = Path::new(path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        for pattern in [
            ".env",
            ".env.*",
            "*.pem",
            "*.key",
            "id_rsa*",
            "credentials*",
            "secrets*",
        ] {
            if glob_matches(pattern, path) || glob_matches(pattern, file_name) {
                if let Some(callback) = confirm_callback.as_deref_mut() {
                    if callback(name, args, &format!("protected path: {pattern}")) {
                        return Ok(());
                    }
                }
                return Err(format!("Permission denied: protected path: {pattern}"));
            }
        }
    }

    Ok(())
}

fn permission_rules<'a>(
    permissions: &'a Map<String, Value>,
    key: &str,
) -> Vec<&'a Map<String, Value>> {
    permissions
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .collect()
}

fn rule_matches(rule: &Map<String, Value>, tool_name: &str, args: &Map<String, Value>) -> bool {
    if let Some(tool_pattern) = rule.get("tool").and_then(Value::as_str) {
        if tool_pattern != "*" && !glob_matches(tool_pattern, tool_name) {
            return false;
        }
    }

    if let Some(paths) = rule.get("paths").and_then(Value::as_array) {
        let path = args.get("path").and_then(Value::as_str).unwrap_or("");
        let file_name = Path::new(path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if !paths
            .iter()
            .filter_map(Value::as_str)
            .any(|pattern| glob_matches(pattern, path) || glob_matches(pattern, file_name))
        {
            return false;
        }
    }

    if let Some(commands) = rule.get("commands").and_then(Value::as_array) {
        let command = args
            .get("command")
            .and_then(Value::as_str)
            .or_else(|| args.get("args").and_then(Value::as_str))
            .unwrap_or("")
            .to_lowercase();
        if !commands
            .iter()
            .filter_map(Value::as_str)
            .any(|pattern| command.contains(&pattern.to_lowercase()))
        {
            return false;
        }
    }

    true
}

fn glob_matches(pattern: &str, text: &str) -> bool {
    fn inner(pattern: &[u8], text: &[u8]) -> bool {
        if pattern.is_empty() {
            return text.is_empty();
        }
        match pattern[0] {
            b'*' => inner(&pattern[1..], text) || (!text.is_empty() && inner(pattern, &text[1..])),
            b'?' => !text.is_empty() && inner(&pattern[1..], &text[1..]),
            current => !text.is_empty() && current == text[0] && inner(&pattern[1..], &text[1..]),
        }
    }

    inner(pattern.as_bytes(), text.as_bytes())
}

fn bytes_to_gb(bytes: u64) -> f64 {
    ((bytes as f64) / 1024_f64.powi(3) * 10.0).round() / 10.0
}

fn normalize_system_name(name: &str) -> String {
    match name {
        "macos" => "macOS".to_owned(),
        "linux" => "Linux".to_owned(),
        "windows" => "Windows".to_owned(),
        other => other.to_owned(),
    }
}

fn ollama_base_url() -> String {
    let raw = env::var("OLLAMA_HOST")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:11434".to_owned());
    let trimmed = raw.trim().trim_end_matches('/').to_owned();
    if trimmed.contains("://") {
        trimmed
    } else {
        format!("http://{trimmed}")
    }
}

fn detect_total_ram_bytes() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        let text = String::from_utf8(output.stdout).ok()?;
        return text.trim().parse::<u64>().ok();
    }

    #[cfg(target_os = "linux")]
    {
        let meminfo = fs::read_to_string("/proc/meminfo").ok()?;
        for line in meminfo.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                let kb = rest.split_whitespace().next()?.parse::<u64>().ok()?;
                return Some(kb * 1024);
            }
        }
        return None;
    }

    #[allow(unreachable_code)]
    None
}

fn nearest_existing_path(path: &Path) -> PathBuf {
    let mut current = path.to_path_buf();
    while !current.exists() {
        let Some(parent) = current.parent() else {
            break;
        };
        if parent == current {
            break;
        }
        current = parent.to_path_buf();
    }
    if current.exists() {
        current
    } else {
        home_dir().unwrap_or_else(|| PathBuf::from("."))
    }
}

fn get_storage_free_gb(path: &Path) -> Option<f64> {
    let existing = nearest_existing_path(path);
    available_space(existing).ok().map(bytes_to_gb)
}

fn merge_json(base: &mut Value, override_value: Value) {
    match (base, override_value) {
        (Value::Object(base_map), Value::Object(override_map)) => {
            for (key, value) in override_map {
                if let Some(existing) = base_map.get_mut(&key) {
                    merge_json(existing, value);
                } else {
                    base_map.insert(key, value);
                }
            }
        }
        (base_slot, new_value) => *base_slot = new_value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_model_registry() {
        let registry = load_model_registry().expect("registry");
        assert!(registry.contains_key("qwen2.5-coder:7b"));
        assert!(registry.len() > 5);
    }

    #[test]
    fn model_fit_prefers_disk_limit() {
        let info = ModelInfo {
            summary: "test".into(),
            size_gb: 10.0,
            use_cases: vec!["test".into()],
            tool_support: true,
            min_ram_gb: 8,
            recommended_ram_gb: 12,
            source: "ollama".into(),
            gguf: None,
        };
        let profile = SystemProfile {
            system: "macOS".into(),
            machine: "arm64".into(),
            cpu_count: 8,
            total_ram_gb: Some(64.0),
            storage_path: PathBuf::from("/tmp"),
            storage_free_gb: Some(8.0),
        };
        assert_eq!(model_fit(&info, &profile), ModelFit::TightDisk);
    }

    #[test]
    fn token_usage_uses_runtime_context_window() {
        let usage = token_usage(
            "qwen2.5-coder:32b",
            &[
                ChatMessage::user("x".repeat(4096)),
                ChatMessage::assistant("done"),
            ],
        );
        assert_eq!(usage.context_window, 131_072);
        assert_eq!(usage.runtime_context_window, 8_192);
        assert_eq!(usage.limit, 8_192);
        assert!(usage.used > 0);
        assert!(usage.remaining < usage.limit);
    }

    #[test]
    fn run_hooks_executes_configured_scripts_with_context() {
        let root = env::temp_dir().join(format!("vibn-hooks-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&root).expect("mkdir");
        let script = root.join("hook.sh");
        let output = root.join("hook.out");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s\\n%s\\n' \"$VIBN_EVENT\" \"$VIBN_CONTEXT\" > \"{}\"\n",
                output.display()
            ),
        )
        .expect("write script");

        let mut config = load_config().expect("config");
        config.extra.insert(
            "hooks".to_owned(),
            json!({
                HOOK_PRE_CHAT: format!("sh {}", script.display()),
            }),
        );
        let mut context = Map::new();
        context.insert(
            "message".to_owned(),
            Value::String("hello hooks".to_owned()),
        );

        let results = run_hooks(&config, HOOK_PRE_CHAT, Some(&context));

        assert_eq!(results.len(), 1);
        let rendered = fs::read_to_string(&output).expect("read output");
        assert!(rendered.contains(HOOK_PRE_CHAT));
        assert!(rendered.contains("\"message\":\"hello hooks\""));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn retry_with_backoff_retries_until_success() {
        let mut attempts = 0usize;
        let result = retry_with_backoff(
            3,
            Duration::ZERO,
            || -> Result<&'static str, &'static str> {
                attempts += 1;
                if attempts < 3 {
                    Err("temporary failure")
                } else {
                    Ok("ok")
                }
            },
        );

        assert_eq!(result, Ok("ok"));
        assert_eq!(attempts, 3);
    }

    #[test]
    fn merges_nested_json_objects() {
        let mut base = serde_json::json!({
            "permissions": {"always_allow": [], "confirm": []},
            "default_model": "a"
        });
        let override_value = serde_json::json!({
            "permissions": {"always_allow": ["run_command"]},
            "default_model": "b"
        });
        merge_json(&mut base, override_value);
        assert_eq!(base["default_model"], "b");
        assert_eq!(base["permissions"]["always_allow"][0], "run_command");
        assert!(base["permissions"]["confirm"].is_array());
    }

    #[test]
    fn command_timeout_defaults_to_120() {
        let config = AppConfig {
            schema_version: 1,
            default_model: "qwen2.5-coder:7b".into(),
            ollama_models_path: String::new(),
            extra: Map::new(),
        };
        assert_eq!(config.command_timeout_secs(), 120);
    }

    #[test]
    fn transcript_round_trip_works() {
        let session_id = format!("test_{}", Uuid::new_v4().simple());
        let messages = vec![ChatMessage::user("hello")];
        save_transcript(&session_id, &messages, None).expect("save");
        let (metadata, loaded) = load_transcript(&session_id).expect("load");
        assert!(metadata.is_some());
        assert_eq!(loaded.len(), 1);
        let path = vibn_transcripts_dir().join(format!("{session_id}.jsonl"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn destructive_commands_are_blocked() {
        let config = load_config().expect("config");
        let args = serde_json::json!({"command": "rm -rf target"});
        let result = execute_tool(
            "run_command",
            args.as_object().expect("object"),
            &config,
            Path::new("."),
        );
        assert!(result.is_err());
    }

    #[test]
    fn read_file_numbers_lines() {
        let temp_path = env::temp_dir().join(format!("vibn-read-{}.txt", Uuid::new_v4().simple()));
        fs::write(&temp_path, "first\nsecond\n").expect("write temp");
        let config = load_config().expect("config");
        let args = serde_json::json!({"path": temp_path.display().to_string()});
        let result = execute_tool(
            "read_file",
            args.as_object().expect("object"),
            &config,
            Path::new("."),
        )
        .expect("read");
        assert!(result.contains("   1: first"));
        assert!(result.contains("   2: second"));
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn find_files_returns_relative_matches() {
        let root = env::temp_dir().join(format!("vibn-find-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(root.join("src")).expect("mkdir");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write file");
        let config = load_config().expect("config");
        let args = serde_json::json!({"pattern": "**/*.rs", "path": root.display().to_string()});
        let result = execute_tool(
            "find_files",
            args.as_object().expect("object"),
            &config,
            Path::new("."),
        )
        .expect("find");
        assert!(result.contains("src/main.rs"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_directory_respects_recursive_depth() {
        let root = env::temp_dir().join(format!("vibn-tree-{}", Uuid::new_v4().simple()));
        let deep = root.join("a/b/c/d");
        fs::create_dir_all(&deep).expect("mkdir");
        let config = load_config().expect("config");
        let args = serde_json::json!({
            "path": root.display().to_string(),
            "recursive": true,
            "depth": 2
        });
        let result = execute_tool(
            "list_directory",
            args.as_object().expect("object"),
            &config,
            Path::new("."),
        )
        .expect("list");
        assert!(result.contains("a/"));
        assert!(result.contains("b/"));
        assert!(!result.contains("c/"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fallback_search_code_finds_line_matches() {
        let root = env::temp_dir().join(format!("vibn-search-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(root.join("src")).expect("mkdir");
        fs::write(root.join("src/lib.rs"), "pub fn vibn_search() {}\n").expect("write file");
        let result = fallback_search_code("vibn_search", &root, Some("**/*.rs")).expect("search");
        assert!(result.contains("src/lib.rs:1: pub fn vibn_search() {}"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn write_edit_and_patch_tools_modify_files() {
        let root = env::temp_dir().join(format!("vibn-edit-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&root).expect("mkdir");
        let path = root.join("file.txt");
        let config = load_config().expect("config");

        let write_args = serde_json::json!({
            "path": path.display().to_string(),
            "content": "alpha\nbeta\n"
        });
        execute_tool(
            "write_file",
            write_args.as_object().expect("object"),
            &config,
            Path::new("."),
        )
        .expect("write");

        let edit_args = serde_json::json!({
            "path": path.display().to_string(),
            "old_string": "beta",
            "new_string": "gamma"
        });
        execute_tool(
            "edit_file",
            edit_args.as_object().expect("object"),
            &config,
            Path::new("."),
        )
        .expect("edit");

        let patch_args = serde_json::json!({
            "path": path.display().to_string(),
            "edits": [{"old": "alpha", "new": "omega"}]
        });
        execute_tool(
            "patch_file",
            patch_args.as_object().expect("object"),
            &config,
            Path::new("."),
        )
        .expect("patch");

        let content = fs::read_to_string(&path).expect("read file");
        assert_eq!(content, "omega\ngamma\n");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn protected_paths_are_blocked_for_writes() {
        let config = load_config().expect("config");
        let args = serde_json::json!({
            "path": ".env",
            "content": "SECRET=1\n"
        });
        let result = execute_tool(
            "write_file",
            args.as_object().expect("object"),
            &config,
            Path::new("."),
        );
        assert!(result.is_err());
    }

    #[test]
    fn git_tool_runs_against_repo() {
        let root = env::temp_dir().join(format!("vibn-git-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&root).expect("mkdir");
        Command::new("git")
            .args(["init"])
            .current_dir(&root)
            .output()
            .expect("git init");
        let config = load_config().expect("config");
        let args = serde_json::json!({"args": "status --short"});
        let result = execute_tool("git", args.as_object().expect("object"), &config, &root)
            .expect("git status");
        assert!(result == "[no output]" || result.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn observation_round_trip_works() {
        let project = env::temp_dir().join(format!("vibn-obs-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&project).expect("mkdir");
        let config = load_config().expect("config");
        let text = format!("observation {}", Uuid::new_v4().simple());

        let save_args = serde_json::json!({"text": text, "scope": "project"});
        execute_tool(
            "save_observation",
            save_args.as_object().expect("object"),
            &config,
            &project,
        )
        .expect("save observation");

        let read_args = serde_json::json!({});
        let content = execute_tool(
            "read_observations",
            read_args.as_object().expect("object"),
            &config,
            &project,
        )
        .expect("read observations");
        assert!(content.contains(&text));

        let _ = fs::remove_file(project_observations_path(&project));
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn forget_project_memory_removes_selected_entry() {
        let project = env::temp_dir().join(format!("vibn-forget-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&project).expect("mkdir");
        let config = load_config().expect("config");

        for text in ["first memory", "second memory"] {
            let save_args = serde_json::json!({"text": text, "scope": "project"});
            execute_tool(
                "save_observation",
                save_args.as_object().expect("object"),
                &config,
                &project,
            )
            .expect("save observation");
        }

        let removed = forget_project_memory(&project, 1)
            .expect("forget")
            .expect("removed");
        assert_eq!(removed.text, "first memory");

        let remaining = project_memory_entries(&project).expect("entries");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].text, "second memory");

        let _ = fs::remove_file(project_observations_path(&project));
        let _ = fs::remove_dir_all(project);
    }

    #[test]
    fn parses_textual_tool_call_fallback() {
        let calls = parse_tool_calls_from_text(
            "```json\n{\"name\":\"list_directory\",\"arguments\":{\"path\":\"./crates\",\"recursive\":false}}\n```",
        );
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "list_directory");
    }

    #[test]
    fn parses_inline_tool_call_fallback() {
        let calls = parse_tool_calls_from_text(
            "list_directory {\"path\":\"./crates\",\"recursive\":false}",
        );
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "list_directory");
    }

    #[test]
    fn file_cache_invalidates_after_write() {
        let root = env::temp_dir().join(format!("vibn-cache-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&root).expect("mkdir");
        let path = root.join("file.txt");
        fs::write(&path, "first\n").expect("write");
        let config = load_config().expect("config");
        let mut cache = FileStateCache::default();
        let mut no_confirm: Option<&mut ConfirmCallbackFn> = None;
        let mut no_diff: Option<&mut DiffCallbackFn> = None;

        let read_args = serde_json::json!({"path": path.display().to_string()});
        let first = execute_agent_tool(
            &mut cache,
            "read_file",
            read_args.as_object().expect("object"),
            &config,
            Path::new("."),
            &mut no_confirm,
            &mut no_diff,
        );
        assert!(first.contains("first"));

        let write_args =
            serde_json::json!({"path": path.display().to_string(), "content": "second\n"});
        let _ = execute_agent_tool(
            &mut cache,
            "write_file",
            write_args.as_object().expect("object"),
            &config,
            Path::new("."),
            &mut no_confirm,
            &mut no_diff,
        );

        let second = execute_agent_tool(
            &mut cache,
            "read_file",
            read_args.as_object().expect("object"),
            &config,
            Path::new("."),
            &mut no_confirm,
            &mut no_diff,
        );
        assert!(second.contains("second"));
        let _ = fs::remove_dir_all(root);
    }
}
