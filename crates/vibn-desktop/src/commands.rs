use std::path::PathBuf;
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use vibn_core::{
    AppConfig, ChatMessage, McpServerStatus, ModelInfo, ObservationEntry, OllamaClient, TokenUsage,
    load_config, load_model_registry, new_session_id as core_new_session_id, save_config,
};

fn ollama_base_url() -> String {
    std::env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".to_owned())
}

#[derive(Debug, Serialize)]
pub struct ModelEntry {
    pub name: String,
    pub size: u64,
    pub modified_at: String,
}

#[tauri::command]
pub fn list_models() -> Result<Vec<ModelEntry>, String> {
    let url = format!("{}/api/tags", ollama_base_url());
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp: Value = client
        .get(url)
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    if let Some(arr) = resp.get("models").and_then(Value::as_array) {
        for entry in arr {
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            if name.is_empty() {
                continue;
            }
            out.push(ModelEntry {
                name,
                size: entry.get("size").and_then(Value::as_u64).unwrap_or(0),
                modified_at: entry
                    .get("modified_at")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
            });
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn active_model() -> Result<String, String> {
    let config = load_config().map_err(|e| e.to_string())?;
    Ok(config.default_model)
}

#[tauri::command]
pub fn set_active_model(model: String) -> Result<(), String> {
    let mut config = load_config().map_err(|e| e.to_string())?;
    config.default_model = model;
    save_config(&config).map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
pub struct TranscriptSummary {
    pub session_id: String,
    pub timestamp: String,
    pub model: String,
    pub project: String,
    pub messages: usize,
    pub title: String,
    pub preview: String,
    pub project_label: String,
}

fn truncate_chars(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn first_nonempty_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_owned()
}

fn project_basename(project: &str) -> String {
    if project.is_empty() {
        return String::new();
    }
    std::path::Path::new(project)
        .file_name()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| project.to_owned())
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_transcripts(limit: Option<usize>) -> Result<Vec<TranscriptSummary>, String> {
    let limit = limit.unwrap_or(50);
    let entries = vibn_core::list_transcripts(limit).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(entries.len());
    for e in entries {
        let first_user = vibn_core::load_transcript(&e.session_id)
            .ok()
            .and_then(|(_meta, msgs)| {
                msgs.into_iter()
                    .find(|m| m.role == "user")
                    .map(|m| m.content)
            })
            .unwrap_or_default();
        let line = first_nonempty_line(&first_user);
        let project_label = project_basename(&e.project);
        let title = if !line.is_empty() {
            truncate_chars(&line, 60)
        } else if !project_label.is_empty() {
            format!("{project_label} session")
        } else {
            "Untitled chat".to_owned()
        };
        let preview = truncate_chars(&line, 120);
        out.push(TranscriptSummary {
            session_id: e.session_id,
            timestamp: e.timestamp,
            model: e.model,
            project: e.project,
            messages: e.messages,
            title,
            preview,
            project_label,
        });
    }
    Ok(out)
}

#[derive(Debug, Serialize)]
pub struct SlashCommandEntry {
    pub command: String,
    pub description: String,
}

#[tauri::command]
pub fn list_slash_commands() -> Vec<SlashCommandEntry> {
    vibn_core::slash_command_definitions()
        .iter()
        .map(|d| SlashCommandEntry {
            command: d.command.trim().to_owned(),
            description: d.description.to_owned(),
        })
        .collect()
}

#[tauri::command]
pub async fn install_comfyui_cmd(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    use tauri::Emitter;
    tauri::async_runtime::spawn_blocking(move || {
        let app_for_progress = app.clone();
        let mut log = Vec::new();
        let result = vibn_core::install_comfyui(|msg| {
            log.push(msg.to_owned());
            let _ = app_for_progress.emit("vibn://install-progress", msg.to_owned());
        });
        match result {
            Ok(()) => {
                log.push("Done.".to_owned());
                let _ = app.emit("vibn://install-progress", "Done.".to_owned());
                Ok(log)
            }
            Err(e) => Err(e),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn start_comfyui_cmd() -> Result<String, String> {
    let config = load_config().map_err(|e| e.to_string())?;
    vibn_core::start_comfyui(&config)?;
    Ok("ComfyUI starting in background.".to_owned())
}

#[tauri::command]
pub fn stop_comfyui_cmd() -> Result<String, String> {
    vibn_core::stop_comfyui()
}

#[tauri::command(rename_all = "snake_case")]
pub async fn download_image_model_cmd(
    app: tauri::AppHandle,
    model_key: String,
) -> Result<Vec<String>, String> {
    use tauri::Emitter;
    tauri::async_runtime::spawn_blocking(move || {
        let app_for_progress = app.clone();
        let mut log = Vec::new();
        let result = vibn_core::download_checkpoint_for(&model_key, |msg| {
            log.push(msg.to_owned());
            let _ = app_for_progress.emit("vibn://install-progress", msg.to_owned());
        });
        match result {
            Ok(_) => {
                log.push("Done.".to_owned());
                let _ = app.emit("vibn://install-progress", "Done.".to_owned());
                Ok(log)
            }
            Err(e) => Err(e),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Debug, Serialize)]
pub struct TranscriptPayload {
    pub session_id: String,
    pub model: String,
    pub project: String,
    pub timestamp: String,
    pub messages: Vec<ChatMessage>,
}

#[tauri::command(rename_all = "snake_case")]
pub fn load_transcript(session_id: String) -> Result<TranscriptPayload, String> {
    let (meta, messages) = vibn_core::load_transcript(&session_id).map_err(|e| e.to_string())?;
    let (model, project, timestamp) = match meta {
        Some(m) => (
            m.extra
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            m.extra
                .get("project")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            m.timestamp,
        ),
        None => (String::new(), String::new(), String::new()),
    };
    Ok(TranscriptPayload {
        session_id,
        model,
        project,
        timestamp,
        messages,
    })
}

#[derive(Debug, Deserialize)]
pub struct SendMessageInput {
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SendMessageOutput {
    pub message: ChatMessage,
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub session_id: String,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn send_message(input: SendMessageInput) -> Result<SendMessageOutput, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let config = load_config().map_err(|e| e.to_string())?;
        let model = input
            .model
            .unwrap_or_else(|| config.default_model.clone());
        let session_id = input
            .session_id
            .unwrap_or_else(vibn_core::new_session_id);
        let client = OllamaClient::new(Duration::from_secs(600)).map_err(|e| e.to_string())?;
        let cwd = agent_cwd();
        let result = vibn_core::run_agent_turns(&client, &model, input.messages, &config, &cwd)
            .map_err(|e| e.to_string())?;

        // Persist the transcript so it shows in the sidebar across launches.
        let mut metadata = Map::new();
        metadata.insert("model".into(), Value::String(model.clone()));
        metadata.insert("mode".into(), Value::String("agent".into()));
        metadata.insert("project".into(), Value::String(cwd.display().to_string()));
        let _ = vibn_core::save_transcript(&session_id, &result.messages, Some(metadata));

        let final_message = result
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "assistant")
            .cloned()
            .unwrap_or_else(|| ChatMessage::assistant(result.final_text.clone()));
        Ok::<SendMessageOutput, String>(SendMessageOutput {
            message: final_message,
            model,
            messages: result.messages,
            session_id,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Debug, Serialize)]
pub struct ConfigPayload {
    pub schema_version: u32,
    pub default_model: String,
    pub ollama_models_path: String,
    pub extra: Map<String, Value>,
}

impl From<AppConfig> for ConfigPayload {
    fn from(c: AppConfig) -> Self {
        Self {
            schema_version: c.schema_version,
            default_model: c.default_model,
            ollama_models_path: c.ollama_models_path,
            extra: c.extra,
        }
    }
}

#[tauri::command]
pub fn get_config() -> Result<ConfigPayload, String> {
    let config = load_config().map_err(|e| e.to_string())?;
    Ok(config.into())
}

#[derive(Debug, Deserialize)]
pub struct SetFieldInput {
    pub key: String,
    pub value: Value,
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_config_field(input: SetFieldInput) -> Result<ConfigPayload, String> {
    let mut config = load_config().map_err(|e| e.to_string())?;
    match input.key.as_str() {
        "default_model" => {
            config.default_model = input
                .value
                .as_str()
                .ok_or("default_model must be a string")?
                .to_owned();
        }
        "ollama_models_path" => {
            config.ollama_models_path = input
                .value
                .as_str()
                .ok_or("ollama_models_path must be a string")?
                .to_owned();
        }
        other => {
            config.extra.insert(other.to_owned(), input.value);
        }
    }
    save_config(&config).map_err(|e| e.to_string())?;
    Ok(config.into())
}

#[tauri::command(rename_all = "snake_case")]
pub fn read_image_as_data_url(path: String) -> Result<String, String> {
    let path = PathBuf::from(&path);
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let mime = mime_guess::from_path(&path)
        .first_or_octet_stream()
        .essence_str()
        .to_owned();
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{encoded}"))
}

#[tauri::command]
pub fn new_session_id() -> String {
    core_new_session_id()
}

#[tauri::command]
pub async fn copy_image_to_clipboard(path: String) -> Result<(), String> {
    use std::process::Command;
    // AppleScript reads the file as PNG data and stuffs it on the pasteboard
    // so a Cmd-V into any app gets the actual image, not a path string.
    let escaped = path.replace('\\', r"\\").replace('"', r#"\""#);
    let script = format!(
        "set the clipboard to (read (POSIX file \"{escaped}\") as «class PNGf»)"
    );
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let out = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|e| format!("Failed to invoke osascript: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn save_image_as(
    app: tauri::AppHandle,
    src_path: String,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let suggested = std::path::Path::new(&src_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "image.png".to_owned());

    let chosen = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_file_name(&suggested)
            .blocking_save_file()
    })
    .await
    .map_err(|e| e.to_string())?;

    let Some(target) = chosen else {
        return Ok(None);
    };
    let target_path = target
        .into_path()
        .map_err(|e| format!("Bad save destination: {e}"))?;
    std::fs::copy(&src_path, &target_path)
        .map_err(|e| format!("Failed to write file: {e}"))?;
    Ok(Some(target_path.to_string_lossy().to_string()))
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_transcript(session_id: String) -> Result<(), String> {
    let path = vibn_core::vibn_transcripts_dir().join(format!("{session_id}.jsonl"));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ===== MCP =====

#[derive(Debug, Serialize)]
pub struct McpServerEntry {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub tool_count: usize,
    pub connected: bool,
}

impl From<McpServerStatus> for McpServerEntry {
    fn from(s: McpServerStatus) -> Self {
        Self {
            name: s.name,
            command: s.command,
            args: s.args,
            tool_count: s.tool_count,
            connected: true,
        }
    }
}

#[tauri::command]
pub fn list_mcp_servers() -> Result<Vec<McpServerEntry>, String> {
    let config = load_config().map_err(|e| e.to_string())?;
    let _ = vibn_core::sync_mcp_servers_from_config(&config);
    let connected = vibn_core::list_connected_mcp_servers().unwrap_or_default();
    let connected_names: std::collections::HashSet<String> =
        connected.iter().map(|s| s.name.clone()).collect();

    let mut out: Vec<McpServerEntry> = connected.into_iter().map(McpServerEntry::from).collect();
    if let Some(servers) = config.extra.get("mcp_servers").and_then(Value::as_object) {
        for (name, cfg) in servers {
            if connected_names.contains(name) {
                continue;
            }
            let command = cfg
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let args = cfg
                .get("args")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            out.push(McpServerEntry {
                name: name.clone(),
                command,
                args,
                tool_count: 0,
                connected: false,
            });
        }
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
pub struct McpAddInput {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[tauri::command(rename_all = "snake_case")]
pub fn add_mcp_server(input: McpAddInput) -> Result<(), String> {
    let mut config = load_config().map_err(|e| e.to_string())?;
    let mut servers = config
        .extra
        .get("mcp_servers")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    servers.insert(
        input.name.clone(),
        serde_json::json!({"command": input.command, "args": input.args}),
    );
    config
        .extra
        .insert("mcp_servers".to_owned(), Value::Object(servers));
    save_config(&config).map_err(|e| e.to_string())?;
    vibn_core::connect_mcp_server(&input.name, &input.command, &input.args, &Map::new())?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn remove_mcp_server(name: String) -> Result<(), String> {
    let _ = vibn_core::disconnect_mcp_server(&name);
    let mut config = load_config().map_err(|e| e.to_string())?;
    if let Some(servers) = config.extra.get_mut("mcp_servers").and_then(Value::as_object_mut) {
        servers.remove(&name);
    }
    save_config(&config).map_err(|e| e.to_string())
}

// ===== Project memory / observations =====

#[derive(Debug, Serialize)]
pub struct MemoryEntry {
    pub heading: String,
    pub text: String,
}

impl From<ObservationEntry> for MemoryEntry {
    fn from(e: ObservationEntry) -> Self {
        Self {
            heading: e.heading,
            text: e.text,
        }
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_project_memory(project_path: Option<String>) -> Result<Vec<MemoryEntry>, String> {
    let path = project_path
        .map(PathBuf::from)
        .unwrap_or_else(agent_cwd);
    let entries = vibn_core::project_memory_entries(&path).map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(MemoryEntry::from).collect())
}

#[derive(Debug, Deserialize)]
pub struct SaveObservationInput {
    pub text: String,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub project_path: Option<String>,
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_observation(input: SaveObservationInput) -> Result<String, String> {
    let cwd = input
        .project_path
        .map(PathBuf::from)
        .unwrap_or_else(agent_cwd);
    let scope = input.scope.unwrap_or_else(|| "project".to_owned());
    let mut args = Map::new();
    args.insert("text".to_owned(), Value::String(input.text));
    args.insert("scope".to_owned(), Value::String(scope));
    let config = load_config().map_err(|e| e.to_string())?;
    vibn_core::execute_tool("save_observation", &args, &config, &cwd)
}

#[derive(Debug, Deserialize)]
pub struct ForgetMemoryInput {
    pub index: usize,
    #[serde(default)]
    pub project_path: Option<String>,
}

#[tauri::command(rename_all = "snake_case")]
pub fn forget_project_memory(input: ForgetMemoryInput) -> Result<bool, String> {
    let cwd = input
        .project_path
        .map(PathBuf::from)
        .unwrap_or_else(agent_cwd);
    Ok(vibn_core::forget_project_memory(&cwd, input.index)?.is_some())
}

// ===== Token usage =====

#[derive(Debug, Serialize)]
pub struct TokenUsagePayload {
    pub used: usize,
    pub limit: usize,
    pub percent: f64,
    pub remaining: usize,
    pub context_window: usize,
}

impl From<TokenUsage> for TokenUsagePayload {
    fn from(t: TokenUsage) -> Self {
        Self {
            used: t.used,
            limit: t.limit,
            percent: t.percent,
            remaining: t.remaining,
            context_window: t.context_window,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TokenUsageInput {
    pub model: String,
    pub messages: Vec<ChatMessage>,
}

#[tauri::command(rename_all = "snake_case")]
pub fn token_usage(input: TokenUsageInput) -> TokenUsagePayload {
    vibn_core::token_usage(&input.model, &input.messages).into()
}

// ===== Model registry =====

#[derive(Debug, Serialize)]
pub struct RegistryModel {
    pub key: String,
    pub summary: String,
    pub size_gb: f64,
    pub use_cases: Vec<String>,
    pub tool_support: bool,
    pub vision: bool,
    pub source: String,
    pub min_ram_gb: u32,
    pub recommended_ram_gb: u32,
}

impl RegistryModel {
    fn from(key: String, info: ModelInfo) -> Self {
        Self {
            key,
            summary: info.summary,
            size_gb: info.size_gb,
            use_cases: info.use_cases,
            tool_support: info.tool_support,
            vision: info.vision,
            source: info.source,
            min_ram_gb: info.min_ram_gb,
            recommended_ram_gb: info.recommended_ram_gb,
        }
    }
}

#[tauri::command]
pub fn list_model_registry() -> Result<Vec<RegistryModel>, String> {
    let registry = load_model_registry().map_err(|e| e.to_string())?;
    Ok(registry
        .into_iter()
        .map(|(k, v)| RegistryModel::from(k, v))
        .collect())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn pull_ollama_model(app: tauri::AppHandle, model: String) -> Result<String, String> {
    use std::io::{BufRead, BufReader};
    use tauri::Emitter;
    tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let url = format!("{}/api/pull", ollama_base_url());
        // Pulls can take minutes; disable the global timeout so the streaming
        // body isn't cut off mid-download.
        let client = reqwest::blocking::Client::builder()
            .timeout(None)
            .build()
            .map_err(|e| e.to_string())?;
        let _ = app.emit(
            "vibn://install-progress",
            format!("Pulling {model} via Ollama…"),
        );
        let resp = client
            .post(&url)
            .json(&serde_json::json!({ "name": model, "stream": true }))
            .send()
            .map_err(|e| {
                format!(
                    "Could not reach Ollama at {}. Is the Ollama app running? ({e})",
                    ollama_base_url()
                )
            })?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(format!("Ollama pull failed ({status}): {body}"));
        }
        let reader = BufReader::new(resp);
        for line in reader.lines() {
            let line = line.map_err(|e| e.to_string())?;
            if line.trim().is_empty() {
                continue;
            }
            let Ok(payload) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if let Some(err) = payload.get("error").and_then(Value::as_str) {
                return Err(err.to_string());
            }
            let status = payload
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let total = payload.get("total").and_then(Value::as_u64);
            let completed = payload.get("completed").and_then(Value::as_u64);
            let msg = match (total, completed) {
                (Some(t), Some(c)) if t > 0 => {
                    let pct = ((c as f64 / t as f64) * 100.0).round() as u64;
                    format!("{status} — {pct}%")
                }
                _ => status,
            };
            if !msg.trim().is_empty() {
                let _ = app.emit("vibn://install-progress", msg);
            }
        }
        let _ = app.emit("vibn://install-progress", format!("Pulled {model}."));
        Ok(format!("Pulled {model}."))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ===== Generic slash command exec (read-only commands) =====

#[derive(Debug, Deserialize)]
pub struct RunSlashInput {
    pub command: String,
    #[serde(default)]
    pub args: String,
}

#[tauri::command(rename_all = "snake_case")]
pub fn run_slash_text(input: RunSlashInput) -> Result<String, String> {
    let config = load_config().map_err(|e| e.to_string())?;
    let cwd = agent_cwd();
    let cmd = input.command.trim_start_matches('/').to_owned();
    let arg = input.args.trim().to_owned();
    let mut args = Map::new();
    match cmd.as_str() {
        "tree" => {
            args.insert("path".into(), Value::String(".".into()));
            args.insert("recursive".into(), Value::Bool(true));
            let depth = arg.parse::<i64>().unwrap_or(3);
            args.insert("depth".into(), Value::Number(depth.into()));
            vibn_core::execute_tool("list_directory", &args, &config, &cwd)
        }
        "search" => {
            if arg.is_empty() {
                return Err("usage: /search PATTERN".into());
            }
            args.insert("pattern".into(), Value::String(arg));
            args.insert("path".into(), Value::String(".".into()));
            vibn_core::execute_tool("search_code", &args, &config, &cwd)
        }
        "find" => {
            if arg.is_empty() {
                return Err("usage: /find GLOB".into());
            }
            args.insert("pattern".into(), Value::String(arg));
            args.insert("path".into(), Value::String(".".into()));
            vibn_core::execute_tool("find_files", &args, &config, &cwd)
        }
        "git" | "diff" => {
            let args_str = if cmd == "diff" { "diff".to_owned() } else { arg };
            args.insert("args".into(), Value::String(args_str));
            vibn_core::execute_tool("git", &args, &config, &cwd)
        }
        "memory" => {
            let entries = vibn_core::project_memory_entries(&cwd).map_err(|e| e.to_string())?;
            if entries.is_empty() {
                Ok("No remembered facts for this project.".into())
            } else {
                let mut out = String::new();
                for (i, e) in entries.iter().enumerate() {
                    out.push_str(&format!("{}. {} — {}\n", i + 1, e.heading, e.text));
                }
                Ok(out.trim_end().to_owned())
            }
        }
        "remember" => {
            if arg.is_empty() {
                return Err("usage: /remember TEXT".into());
            }
            args.insert("text".into(), Value::String(arg));
            args.insert("scope".into(), Value::String("project".into()));
            vibn_core::execute_tool("save_observation", &args, &config, &cwd)
        }
        "tokens" => {
            let usage = vibn_core::token_usage(&config.default_model, &[]);
            Ok(format!(
                "model={} window={} runtime_window={}",
                config.default_model, usage.context_window, usage.runtime_context_window
            ))
        }
        "status" => Ok(format!(
            "model = {}\ncwd = {}\nmcp = {}",
            config.default_model,
            cwd.display(),
            vibn_core::connected_mcp_summary()
        )),
        "mcp" => Ok(vibn_core::connected_mcp_summary()),
        other => Err(format!("/{other} is not yet wired into the desktop UI.")),
    }
}

// ============================================================================
// Project mode (VBN-318/319/320/322): pick a directory, remember it across
// launches, and have the agent's cwd follow it.
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectInfo {
    pub path: String,
    pub name: String,
    pub last_opened: String,
    pub ecosystems: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectScanResult {
    pub path: String,
    pub name: String,
    pub ecosystems: Vec<String>,
    pub has_code: bool,
}

#[derive(Debug, Serialize)]
pub struct ActiveProjectState {
    pub active: Option<ProjectInfo>,
    pub recent: Vec<ProjectInfo>,
}

const RECENT_PROJECTS_LIMIT: usize = 12;

fn scan_path_ecosystems(path: &std::path::Path) -> Vec<String> {
    let signals: &[(&str, &str)] = &[
        ("package.json", "node"),
        ("Cargo.toml", "rust"),
        ("pyproject.toml", "python"),
        ("requirements.txt", "python"),
        ("setup.py", "python"),
        ("go.mod", "go"),
        ("composer.json", "php"),
        ("Gemfile", "ruby"),
        ("pom.xml", "jvm"),
        ("build.gradle", "jvm"),
        ("build.gradle.kts", "jvm"),
        ("Package.swift", "swift"),
        ("mix.exs", "elixir"),
        ("deno.json", "deno"),
        ("bun.lock", "bun"),
        ("pnpm-workspace.yaml", "node"),
        (".git", "git"),
    ];
    let mut out: Vec<String> = signals
        .iter()
        .filter(|(file, _)| path.join(file).exists())
        .map(|(_, eco)| (*eco).to_owned())
        .collect();
    out.sort();
    out.dedup();
    out
}

fn project_name_for(path: &std::path::Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn project_info_for(path: &std::path::Path) -> ProjectInfo {
    ProjectInfo {
        path: path.display().to_string(),
        name: project_name_for(path),
        last_opened: chrono::Utc::now().to_rfc3339(),
        ecosystems: scan_path_ecosystems(path),
    }
}

fn read_active_project_state() -> ActiveProjectState {
    let mut state = ActiveProjectState { active: None, recent: Vec::new() };
    if let Ok(config) = load_config() {
        if let Some(v) = config.extra.get("active_project") {
            state.active = serde_json::from_value(v.clone()).ok();
        }
        if let Some(v) = config.extra.get("recent_projects") {
            state.recent = serde_json::from_value(v.clone()).unwrap_or_default();
        }
    }
    state
}

fn write_active_project_state(state: &ActiveProjectState) -> Result<(), String> {
    let mut config = load_config().map_err(|e| e.to_string())?;
    match &state.active {
        Some(p) => {
            config.extra.insert(
                "active_project".into(),
                serde_json::to_value(p).map_err(|e| e.to_string())?,
            );
        }
        None => {
            config.extra.remove("active_project");
        }
    }
    config.extra.insert(
        "recent_projects".into(),
        serde_json::to_value(&state.recent).map_err(|e| e.to_string())?,
    );
    save_config(&config).map_err(|e| e.to_string())
}

/// Returns the cwd the agent should use for a given invocation:
/// active_project from config when set + still exists, else `current_dir()`.
pub fn agent_cwd() -> PathBuf {
    let state = read_active_project_state();
    if let Some(p) = state.active {
        let path = PathBuf::from(&p.path);
        if path.is_dir() {
            return path;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[tauri::command]
pub fn get_active_project() -> Result<ActiveProjectState, String> {
    Ok(read_active_project_state())
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_active_project(path: String) -> Result<ActiveProjectState, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("Path does not exist: {}", path));
    }
    if !p.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }
    let abs = p.canonicalize().map_err(|e| e.to_string())?;
    let info = project_info_for(&abs);
    let mut state = read_active_project_state();
    state.recent.retain(|r| r.path != info.path);
    state.recent.insert(0, info.clone());
    state.recent.truncate(RECENT_PROJECTS_LIMIT);
    state.active = Some(info);
    write_active_project_state(&state)?;
    Ok(state)
}

#[tauri::command]
pub fn clear_active_project() -> Result<ActiveProjectState, String> {
    let mut state = read_active_project_state();
    state.active = None;
    write_active_project_state(&state)?;
    Ok(state)
}

#[tauri::command(rename_all = "snake_case")]
pub fn forget_recent_project(path: String) -> Result<ActiveProjectState, String> {
    let mut state = read_active_project_state();
    state.recent.retain(|r| r.path != path);
    if state.active.as_ref().is_some_and(|a| a.path == path) {
        state.active = None;
    }
    write_active_project_state(&state)?;
    Ok(state)
}

// ============================================================================
// File explorer (VBN-325): gitignore-aware tree, lazy depth-1 expansion.
// ============================================================================

#[derive(Debug, Serialize)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub kind: String, // "file" | "dir"
    pub has_children: bool,
}

fn safe_within(root: &std::path::Path, target: &std::path::Path) -> Result<PathBuf, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("project root unavailable: {e}"))?;
    let resolved = target
        .canonicalize()
        .map_err(|e| format!("path unavailable: {e}"))?;
    if !resolved.starts_with(&root) {
        return Err(format!(
            "path escapes project root ({} not under {})",
            resolved.display(),
            root.display()
        ));
    }
    Ok(resolved)
}

fn list_dir_one_level(dir: &std::path::Path) -> Vec<FileNode> {
    use ignore::WalkBuilder;
    let mut out = Vec::new();
    let walker = WalkBuilder::new(dir)
        .max_depth(Some(1))
        .standard_filters(true)
        .hidden(true)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .require_git(false)
        .build();
    for entry in walker.flatten() {
        if entry.depth() == 0 {
            continue;
        }
        let path = entry.path();
        let name = match path.file_name() {
            Some(s) => s.to_string_lossy().into_owned(),
            None => continue,
        };
        let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());
        let has_children = if is_dir {
            std::fs::read_dir(path)
                .map(|mut it| it.next().is_some())
                .unwrap_or(false)
        } else {
            false
        };
        out.push(FileNode {
            name,
            path: path.display().to_string(),
            kind: if is_dir { "dir".into() } else { "file".into() },
            has_children,
        });
    }
    out.sort_by(|a, b| match (a.kind.as_str(), b.kind.as_str()) {
        ("dir", "file") => std::cmp::Ordering::Less,
        ("file", "dir") => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    out
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_project_files(path: Option<String>) -> Result<Vec<FileNode>, String> {
    let root = agent_cwd();
    let target = match path {
        Some(p) if !p.is_empty() => safe_within(&root, &PathBuf::from(&p))?,
        _ => root.canonicalize().unwrap_or(root.clone()),
    };
    if !target.is_dir() {
        return Err(format!("not a directory: {}", target.display()));
    }
    Ok(list_dir_one_level(&target))
}

#[derive(Debug, Serialize)]
pub struct FileContent {
    pub path: String,
    pub language: String,
    pub content: String,
    pub size_bytes: u64,
    pub truncated: bool,
}

const FILE_READ_LIMIT_BYTES: u64 = 2 * 1024 * 1024; // 2 MB

fn language_for(path: &std::path::Path) -> String {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase());
    match ext.as_deref() {
        Some("ts") | Some("tsx") => "typescript".into(),
        Some("js") | Some("jsx") | Some("mjs") | Some("cjs") => "javascript".into(),
        Some("rs") => "rust".into(),
        Some("py") => "python".into(),
        Some("go") => "go".into(),
        Some("rb") => "ruby".into(),
        Some("php") => "php".into(),
        Some("swift") => "swift".into(),
        Some("kt") | Some("kts") => "kotlin".into(),
        Some("java") => "java".into(),
        Some("c") | Some("h") => "c".into(),
        Some("cpp") | Some("cc") | Some("hpp") => "cpp".into(),
        Some("cs") => "csharp".into(),
        Some("css") => "css".into(),
        Some("scss") | Some("sass") => "scss".into(),
        Some("html") | Some("htm") => "html".into(),
        Some("json") => "json".into(),
        Some("yaml") | Some("yml") => "yaml".into(),
        Some("toml") => "toml".into(),
        Some("md") | Some("mdx") => "markdown".into(),
        Some("sql") => "sql".into(),
        Some("sh") | Some("bash") | Some("zsh") => "shell".into(),
        _ => "plaintext".into(),
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn read_project_file(path: String) -> Result<FileContent, String> {
    let root = agent_cwd();
    let target = safe_within(&root, &PathBuf::from(&path))?;
    let meta = std::fs::metadata(&target).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err(format!("not a file: {}", target.display()));
    }
    let size_bytes = meta.len();
    let truncated = size_bytes > FILE_READ_LIMIT_BYTES;
    let bytes = if truncated {
        use std::io::Read;
        let mut f = std::fs::File::open(&target).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; FILE_READ_LIMIT_BYTES as usize];
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        buf.truncate(n);
        buf
    } else {
        std::fs::read(&target).map_err(|e| e.to_string())?
    };
    let content = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return Err("binary file — open in an external app".into()),
    };
    Ok(FileContent {
        path: target.display().to_string(),
        language: language_for(&target),
        content,
        size_bytes,
        truncated,
    })
}

#[derive(Debug, Serialize)]
pub struct AgentEditorEvent {
    pub kind: String,
    pub payload: Value,
    pub ts: String,
}

/// Drain pending editor events the agent dropped on disk. UI polls this.
#[tauri::command]
pub fn drain_editor_events() -> Result<Vec<AgentEditorEvent>, String> {
    let dir = vibn_core::vibn_events_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(out),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let parsed: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let _ = std::fs::remove_file(&path);
        out.push(AgentEditorEvent {
            kind: name.to_owned(),
            payload: parsed.get("payload").cloned().unwrap_or(Value::Null),
            ts: parsed
                .get("ts")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
        });
    }
    Ok(out)
}

#[tauri::command(rename_all = "snake_case")]
pub fn write_project_file(path: String, content: String) -> Result<(), String> {
    let root = agent_cwd();
    let target = match safe_within(&root, &PathBuf::from(&path)) {
        Ok(p) => p,
        Err(_) => {
            // Path may not exist yet (new file). Resolve its parent instead.
            let pb = PathBuf::from(&path);
            let parent = pb.parent().ok_or("path has no parent")?;
            let _ = safe_within(&root, parent)?;
            pb
        }
    };
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&target, content).map_err(|e| e.to_string())
}

// ============================================================================
// Desktop-use permissions (VBN-330): silent macOS permission checks.
// ============================================================================

#[derive(Debug, Serialize)]
pub struct DesktopPermissions {
    pub accessibility: bool,
    pub screen_recording: bool,
    pub platform: String,
    pub supported: bool,
}

#[cfg(target_os = "macos")]
fn check_accessibility_inner() -> bool {
    use objc2_application_services::AXIsProcessTrusted;
    unsafe { AXIsProcessTrusted() }
}

#[cfg(target_os = "macos")]
fn check_screen_recording_inner() -> bool {
    use objc2_core_graphics::CGPreflightScreenCaptureAccess;
    CGPreflightScreenCaptureAccess()
}

#[tauri::command]
pub fn check_desktop_permissions() -> Result<DesktopPermissions, String> {
    #[cfg(target_os = "macos")]
    {
        Ok(DesktopPermissions {
            accessibility: check_accessibility_inner(),
            screen_recording: check_screen_recording_inner(),
            platform: "macos".into(),
            supported: true,
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(DesktopPermissions {
            accessibility: false,
            screen_recording: false,
            platform: std::env::consts::OS.into(),
            supported: false,
        })
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn open_system_settings_pane(pane: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let url = match pane.as_str() {
            "accessibility" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            "screen_recording" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
            }
            "automation" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation"
            }
            _ => return Err(format!("unknown pane: {pane}")),
        };
        std::process::Command::new("/usr/bin/open")
            .arg(url)
            .status()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pane;
        Err("unsupported on this platform".into())
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn scan_project(path: String) -> Result<ProjectScanResult, String> {
    let p = PathBuf::from(&path);
    if !p.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }
    let abs = p.canonicalize().map_err(|e| e.to_string())?;
    let ecosystems = scan_path_ecosystems(&abs);
    Ok(ProjectScanResult {
        path: abs.display().to_string(),
        name: project_name_for(&abs),
        has_code: !ecosystems.is_empty() && ecosystems.iter().any(|e| e != "git"),
        ecosystems,
    })
}
