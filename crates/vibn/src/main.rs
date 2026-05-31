use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use comfy_table::{Cell, CellAlignment, Table, presets::UTF8_FULL_CONDENSED};
use serde_json::Value;
use vibn_core::{
    ChatMessage, OllamaClient, append_to_transcript, build_system_profile,
    builtin_tool_definitions, connected_mcp_summary, execute_tool, format_system_summary,
    get_ollama_models_path, load_config, load_model_registry, load_transcript, model_fit,
    new_session_id, remembered_facts_block, run_agent_turns, run_agent_turns_streaming,
    save_transcript,
};

mod tui;

const BASE_SYSTEM_PROMPT: &str =
    "You are Vibn — a local AI coding agent. Respond directly, clearly, and helpfully.";

#[derive(Debug, Parser)]
#[command(name = "vibn", about = "Vibn — local AI coding agent")]
struct Cli {
    #[arg(short = 'm', long, help = "Ollama model to use")]
    model: Option<String>,
    #[arg(short = 'c', long = "cd", help = "Start in this directory")]
    cd: Option<String>,
    #[arg(
        short = 's',
        long,
        help = "Named session to load/create (e.g. --session myproject)"
    )]
    session: Option<String>,
    #[arg(long = "list-models", help = "List recommended models")]
    list_models: bool,
    #[arg(long = "list-tools", hide = true)]
    list_tools: bool,
    #[arg(long = "tool", hide = true)]
    tool: Option<String>,
    #[arg(long = "tool-args", default_value = "{}", hide = true)]
    tool_args: String,
    #[arg(long = "agent", hide = true)]
    agent: bool,
    #[arg(
        long = "json-events",
        hide = true,
        help = "Emit one NDJSON event per line on stdout (for embedding inside AppLab)"
    )]
    json_events: bool,
    #[arg(long = "tui", hide = true)]
    tui: bool,
    #[arg(long = "no-project-prompt", help = "Skip the 'create a Vibn project here?' prompt for code directories")]
    no_project_prompt: bool,
    #[arg(help = "Initial prompt (runs non-interactively)")]
    prompt: Vec<String>,
}

const PROJECT_SIGNALS: &[(&str, &str)] = &[
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
];

fn scan_ecosystems(path: &std::path::Path) -> Vec<String> {
    let mut out: Vec<String> = PROJECT_SIGNALS
        .iter()
        .filter(|(file, _)| path.join(file).exists())
        .map(|(_, eco)| (*eco).to_owned())
        .collect();
    out.sort();
    out.dedup();
    out
}

fn maybe_prompt_create_project(cwd: &std::path::Path, skip: bool) {
    use std::io::{BufRead, Write};

    if skip {
        return;
    }
    if !std::io::stdin().is_terminal() {
        return;
    }
    let mut config = match load_config() {
        Ok(c) => c,
        Err(_) => return,
    };
    let cwd_str = cwd.display().to_string();

    // If already active or in recents, just refresh active and return.
    let active_path = config
        .extra
        .get("active_project")
        .and_then(|v| v.get("path"))
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    if active_path.as_deref() == Some(cwd_str.as_str()) {
        return;
    }
    let known_recent = config
        .extra
        .get("recent_projects")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .any(|r| r.get("path").and_then(|p| p.as_str()) == Some(cwd_str.as_str()))
        })
        .unwrap_or(false);

    let ecosystems = scan_ecosystems(cwd);
    if ecosystems.is_empty() {
        return;
    }

    if known_recent {
        // Silently promote to active without prompting.
        register_project_in_config(&mut config, cwd, &ecosystems, true);
        let _ = vibn_core::save_config(&config);
        return;
    }

    let name = cwd
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| cwd_str.clone());
    eprintln!(
        "Detected a {} project at {}",
        ecosystems.join(" + "),
        cwd_str
    );
    eprint!("Create a Vibn project '{name}' here? [Y/n] ");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    if std::io::stdin().lock().read_line(&mut line).is_err() {
        return;
    }
    let answer = line.trim().to_lowercase();
    if answer.is_empty() || answer == "y" || answer == "yes" {
        register_project_in_config(&mut config, cwd, &ecosystems, true);
        if let Err(err) = vibn_core::save_config(&config) {
            eprintln!("error: failed to save project: {err}");
        } else {
            eprintln!("✓ Registered Vibn project '{name}'");
        }
    }
}

fn register_project_in_config(
    config: &mut vibn_core::AppConfig,
    cwd: &std::path::Path,
    ecosystems: &[String],
    set_active: bool,
) {
    use serde_json::json;
    let name = cwd
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| cwd.display().to_string());
    let info = json!({
        "path": cwd.display().to_string(),
        "name": name,
        "last_opened": chrono::Utc::now().to_rfc3339(),
        "ecosystems": ecosystems,
    });
    if set_active {
        config.extra.insert("active_project".to_owned(), info.clone());
    }
    let mut recent: Vec<Value> = config
        .extra
        .get("recent_projects")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    recent.retain(|r| r.get("path").and_then(|p| p.as_str()) != Some(&cwd.display().to_string()));
    recent.insert(0, info);
    recent.truncate(12);
    config
        .extra
        .insert("recent_projects".to_owned(), Value::Array(recent));
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Some(path) = cli.cd.as_deref() {
        if let Err(error) = std::env::set_current_dir(path) {
            eprintln!("error: failed to change directory to {path}: {error}");
            return ExitCode::from(1);
        }
    }

    // Project-mode bootstrap: only relevant when the user is starting an
    // interactive session in a code directory. Skip for tooling/list flags.
    let interactive = !cli.list_models
        && !cli.list_tools
        && cli.tool.is_none()
        && !cli.agent;
    if interactive {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        maybe_prompt_create_project(&cwd, cli.no_project_prompt);
    }

    if cli.list_models {
        match run_list_models() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::from(1)
            }
        }
    } else if cli.list_tools {
        match run_list_tools() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::from(1)
            }
        }
    } else if cli.tool.is_some() {
        match run_tool(cli) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::from(1)
            }
        }
    } else if cli.agent && !cli.prompt.is_empty() {
        if cli.json_events {
            match run_agent_json_events(cli) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("error: {error}");
                    ExitCode::from(1)
                }
            }
        } else {
            match run_agent(cli) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("error: {error}");
                    ExitCode::from(1)
                }
            }
        }
    } else if !cli.prompt.is_empty() {
        match run_prompt(cli) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::from(1)
            }
        }
    } else if cli.tui || cli.prompt.is_empty() {
        match run_tui(cli) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::from(1)
            }
        }
    } else {
        ExitCode::from(1)
    }
}

fn run_list_models() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    let storage_path =
        get_ollama_models_path(&config).unwrap_or_else(|| std::path::PathBuf::from("~/models"));
    let profile = build_system_profile(storage_path);
    let registry = load_model_registry()?;

    println!("System: {}", format_system_summary(&profile));

    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);
    table.set_header(vec![
        "Model",
        "Best for",
        "Tools",
        "Size",
        "RAM",
        "This system",
    ]);

    for (name, info) in registry {
        let best_for = format!("{} · {}", info.summary, info.use_cases.join(", "));
        table.add_row(vec![
            Cell::new(name),
            Cell::new(best_for),
            Cell::new(if info.tool_support { "yes" } else { "no" }),
            Cell::new(format!("{:.1}GB", info.size_gb)).set_alignment(CellAlignment::Right),
            Cell::new(format!("{}GB+", info.recommended_ram_gb)),
            Cell::new(model_fit(&info, &profile).as_str()),
        ]);
    }

    println!("{table}");
    Ok(())
}

fn run_list_tools() -> Result<(), Box<dyn std::error::Error>> {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);
    table.set_header(vec!["Tool", "Description"]);
    for tool in builtin_tool_definitions() {
        table.add_row(vec![Cell::new(tool.name), Cell::new(tool.description)]);
    }
    println!("{table}");
    Ok(())
}

fn run_prompt(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let project_config = tui::load_project_vibn(&cwd);
    let project_context = tui::render_project_context(&project_config);
    let model = resolve_model(cli.model.as_deref(), &config.default_model, &project_config);
    let user_prompt = cli.prompt.join(" ");
    let session_id = cli.session.unwrap_or_else(new_session_id);
    let (metadata, prior_messages) = load_transcript(&session_id)?;
    let client = OllamaClient::new(Duration::from_secs(config.command_timeout_secs()))?;
    let request_messages =
        build_request_messages(prior_messages, &user_prompt, &cwd, &project_context);
    let response = client.chat(&model, request_messages)?;
    let user_message = ChatMessage::user(user_prompt);
    let assistant_message = ChatMessage::assistant(response.clone());

    if metadata.is_some() {
        append_to_transcript(&session_id, &user_message)?;
        append_to_transcript(&session_id, &assistant_message)?;
    } else {
        let mut metadata = serde_json::Map::new();
        metadata.insert("model".into(), serde_json::Value::String(model.clone()));
        metadata.insert("mode".into(), serde_json::Value::String("one-shot".into()));
        metadata.insert(
            "project".into(),
            serde_json::Value::String(std::env::current_dir()?.display().to_string()),
        );
        save_transcript(
            &session_id,
            &[user_message, assistant_message],
            Some(metadata),
        )?;
    }
    println!("{}", response.trim_end());
    Ok(())
}

/// Run the agent loop with structured event emission. Designed to be
/// embedded by AppLab — every event becomes one NDJSON line on stdout
/// (`flush()` after each so the consumer sees deltas live). On any
/// stream/io failure we emit a final `done` with `stopped: "error"` so
/// the parent process can close its UI cleanly.
fn run_agent_json_events(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;

    let config = load_config()?;
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let project_config = tui::load_project_vibn(&cwd);
    let project_context = tui::render_project_context(&project_config);
    let model = resolve_model(cli.model.as_deref(), &config.default_model, &project_config);
    let user_prompt = cli.prompt.join(" ");
    let session_id = cli.session.unwrap_or_else(new_session_id);
    let (_, prior_messages) = load_transcript(&session_id)?;
    let client = OllamaClient::new(Duration::from_secs(config.command_timeout_secs()))?;

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    let emit = |event: serde_json::Value| {
        // Best-effort write; if the consumer pipe is closed we'll see it
        // on the next iteration and just stop emitting.
        let _ = serde_json::to_writer(&mut handle, &event);
        let _ = handle.write_all(b"\n");
        let _ = handle.flush();
    };

    let result = run_agent_turns_streaming(
        &client,
        &model,
        build_request_messages(prior_messages, &user_prompt, &cwd, &project_context),
        &config,
        &cwd,
        emit,
    );

    match result {
        Ok(run) => {
            save_transcript(&session_id, &run.messages, Some(agent_metadata(&model)?))?;
            Ok(())
        }
        Err(error) => {
            let event = serde_json::json!({
                "type": "error",
                "message": format!("{error}"),
            });
            let _ = serde_json::to_writer(&mut handle, &event);
            let _ = handle.write_all(b"\n");
            let done = serde_json::json!({
                "type": "done",
                "stopped": "error",
                "steps": 0,
            });
            let _ = serde_json::to_writer(&mut handle, &done);
            let _ = handle.write_all(b"\n");
            let _ = handle.flush();
            Err(error)
        }
    }
}

fn run_agent(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let project_config = tui::load_project_vibn(&cwd);
    let project_context = tui::render_project_context(&project_config);
    let model = resolve_model(cli.model.as_deref(), &config.default_model, &project_config);
    let user_prompt = cli.prompt.join(" ");
    let session_id = cli.session.unwrap_or_else(new_session_id);
    let (_, prior_messages) = load_transcript(&session_id)?;
    let client = OllamaClient::new(Duration::from_secs(config.command_timeout_secs()))?;
    let result = run_agent_turns(
        &client,
        &model,
        build_request_messages(prior_messages, &user_prompt, &cwd, &project_context),
        &config,
        &cwd,
    )?;

    save_transcript(&session_id, &result.messages, Some(agent_metadata(&model)?))?;
    if let Some(summary) = &result.auto_compact_summary {
        println!("⟳ Auto-compacted context. {summary}");
    }
    println!("{}", result.final_text.trim_end());
    Ok(())
}

fn run_tool(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    let tool_name = cli.tool.ok_or("Missing --tool name")?;
    let value: serde_json::Value = serde_json::from_str(&cli.tool_args)?;
    let args = value
        .as_object()
        .ok_or("--tool-args must be a JSON object")?;
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let output = execute_tool(&tool_name, args, &config, &cwd).map_err(std::io::Error::other)?;
    println!("{output}");
    Ok(())
}

fn run_tui(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    let model = cli.model.unwrap_or_else(|| config.default_model.clone());
    let session_id = cli.session.unwrap_or_else(new_session_id);
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    tui::run_tui(config, model, session_id, cwd)
}

fn build_request_messages(
    prior_messages: Vec<ChatMessage>,
    user_prompt: &str,
    cwd: &std::path::Path,
    project_context: &str,
) -> Vec<ChatMessage> {
    let mut system_prompt = BASE_SYSTEM_PROMPT.to_owned();
    if !project_context.trim().is_empty() {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(project_context.trim());
    }
    if let Ok(Some(block)) = remembered_facts_block(cwd) {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(&block);
    }
    if let Some(block) = pinned_notes_block(load_pins().ok().as_deref().unwrap_or(&[])) {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(&block);
    }
    let mcp_summary = connected_mcp_summary();
    if !mcp_summary.is_empty() {
        system_prompt.push_str("\n\n## Connected MCP Servers\n");
        system_prompt.push_str(&mcp_summary);
    }
    system_prompt.push_str("\n\nWorking directory: ");
    system_prompt.push_str(&cwd.display().to_string());
    let mut messages = vec![ChatMessage::system(system_prompt)];
    messages.extend(
        prior_messages
            .into_iter()
            .filter(|message| message.role != "system"),
    );
    messages.push(ChatMessage::user(user_prompt));
    messages
}

fn resolve_model(
    cli_model: Option<&str>,
    default_model: &str,
    project_config: &serde_json::Map<String, Value>,
) -> String {
    cli_model
        .filter(|model| !model.trim().is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            project_config
                .get("model")
                .and_then(Value::as_str)
                .filter(|model| !model.trim().is_empty())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| default_model.to_owned())
}

fn load_pins() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Ok(Vec::new());
    };
    let path = home.join(".vibn").join("pins.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    let value: Value = serde_json::from_str(&content)?;
    Ok(match value {
        Value::Array(items) => items
            .into_iter()
            .filter_map(|item| match item {
                Value::String(text) => Some(text),
                Value::Object(object) => object
                    .get("text")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    })
}

fn pinned_notes_block(pins: &[String]) -> Option<String> {
    if pins.is_empty() {
        return None;
    }
    let mut lines = vec!["## Pinned Notes (always remember)".to_owned()];
    lines.extend(pins.iter().map(|pin| format!("- {pin}")));
    Some(lines.join("\n"))
}

fn agent_metadata(
    model: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, Box<dyn std::error::Error>> {
    let mut metadata = serde_json::Map::new();
    metadata.insert("model".into(), serde_json::Value::String(model.to_owned()));
    metadata.insert("mode".into(), serde_json::Value::String("agent".into()));
    metadata.insert(
        "project".into(),
        serde_json::Value::String(std::env::current_dir()?.display().to_string()),
    );
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::{build_request_messages, resolve_model};
    use std::fs;

    use serde_json::Value;
    use vibn_core::{ChatMessage, execute_tool, load_config};

    #[test]
    fn request_messages_replace_prior_system_prompt() {
        let messages = build_request_messages(
            vec![ChatMessage::system("old"), ChatMessage::user("earlier")],
            "next",
            std::path::Path::new("."),
            "",
        );

        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].content, "earlier");
        assert_eq!(messages[2].content, "next");
        assert_eq!(messages.len(), 3);
    }

    #[test]
    fn request_messages_include_remembered_facts() {
        let temp = std::env::temp_dir().join(format!(
            "vibn-main-memory-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("timestamp")
                .as_nanos()
        ));
        fs::create_dir_all(&temp).expect("temp dir");
        let config = load_config().expect("config");
        execute_tool(
            "save_observation",
            serde_json::json!({"text": "remembered detail", "scope": "project"})
                .as_object()
                .expect("object"),
            &config,
            &temp,
        )
        .expect("save memory");

        let messages = build_request_messages(Vec::new(), "next", &temp, "");

        assert!(messages[0].content.contains("## Remembered facts"));
        assert!(messages[0].content.contains("- remembered detail"));
        assert!(messages[0].content.contains("Working directory:"));

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn request_messages_include_project_context() {
        let messages = build_request_messages(
            Vec::new(),
            "next",
            std::path::Path::new("."),
            "Custom project prompt\n\nProject constraints:\n- Stay focused",
        );

        assert!(messages[0].content.contains("Custom project prompt"));
        assert!(messages[0].content.contains("Project constraints:"));
    }

    #[test]
    fn resolve_model_prefers_cli_then_project_then_default() {
        let mut project_config = serde_json::Map::new();
        project_config.insert(
            "model".to_owned(),
            Value::String("project-model".to_owned()),
        );

        assert_eq!(
            resolve_model(Some("cli-model"), "default-model", &project_config),
            "cli-model"
        );
        assert_eq!(
            resolve_model(None, "default-model", &project_config),
            "project-model"
        );
        assert_eq!(
            resolve_model(None, "default-model", &serde_json::Map::new()),
            "default-model"
        );
    }
}
