use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Stdout, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode, size,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use regex::Regex;
use serde_json::{Map, Value, json};
use tui_textarea::TextArea;
use vibn_core::{
    AppConfig, ChatMessage, HOOK_POST_COMPACT, HOOK_PRE_COMPACT, HOOK_SESSION_START, OllamaClient,
    SlashCommandDefinition, build_system_profile, connect_mcp_server, connected_mcp_summary,
    disconnect_mcp_server, execute_tool, forget_project_memory, get_ollama_models_path,
    list_connected_mcp_servers, list_transcripts, load_model_registry, load_transcript, model_fit,
    project_memory_entries, remembered_facts_block,
    run_agent_turns_with_callbacks, run_hooks, save_config, save_transcript,
    slash_command_definitions, sync_mcp_servers_from_config, token_usage, vibn_config_file,
    vibn_transcripts_dir,
};

enum UiEvent {
    Append(String),
    SetProcessing(bool),
    BackgroundFinished {
        id: usize,
        status: &'static str,
        summary: String,
    },
    WatchTriggered {
        prompt: String,
        changed: String,
    },
    RequestConfirm {
        tool: String,
        cmd: String,
        reason: String,
        reply: std::sync::mpsc::Sender<bool>,
    },
    RequestDiff {
        path: String,
        diff_lines: Vec<String>,
        reply: std::sync::mpsc::Sender<bool>,
    },
    ClearConfirm,
    ClearDiff,
}

enum InputAction {
    Continue,
    Quit,
}

struct UiState {
    output_lines: Vec<String>,
    scroll_offset: u16,
    auto_scroll: bool,
    processing: bool,
    plan_mode: bool,
    model: String,
    session_id: String,
    cwd: PathBuf,
    config: AppConfig,
    completion_index: usize,
    pending_confirm: Option<PendingConfirm>,
    pending_diff: Option<PendingDiff>,
    input_history: Vec<String>,
    history_index: Option<usize>,
    history_draft: String,
    pending_watch_prompts: Vec<(String, String)>,
    watchers: BTreeMap<String, WatchHandle>,
    background_tasks: Vec<BackgroundTask>,
    next_background_task_id: usize,
    test_cmd: String,
    constraints_mode: ConstraintsMode,
    review_mode: ReviewMode,
    browser_mode: Option<BrowserMode>,
    install_prompt: Option<InstallPromptMode>,
    project_context: String,
}

struct SlashContext {
    items: Vec<CompletionItem>,
    exact_description: Option<String>,
    suggestion_suffix: Option<String>,
}

struct CompletionItem {
    apply_text: String,
    display: String,
    meta: String,
}

struct PendingConfirm {
    tool: String,
    cmd: String,
    reason: String,
    reply: std::sync::mpsc::Sender<bool>,
}

struct PendingDiff {
    path: String,
    diff_lines: Vec<String>,
    reply: std::sync::mpsc::Sender<bool>,
}

struct WatchHandle {
    active: Arc<AtomicBool>,
}

struct BackgroundTask {
    id: usize,
    prompt: String,
    status: &'static str,
    result_summary: String,
}

struct ConstraintsMode {
    active: bool,
    adding: bool,
    editing: bool,
    edit_index: usize,
    selected: usize,
    rules: Vec<String>,
}

struct ReviewMode {
    active: bool,
    examples: Vec<Value>,
    index: usize,
    staging_file: String,
    approved_file: PathBuf,
    approved: usize,
    discarded: usize,
    skipped: usize,
}

struct TranscriptExportArgs {
    min_turns: usize,
    require_tools: bool,
    output: PathBuf,
    stats: bool,
}

struct TranscriptScore {
    user_turns: usize,
    tool_calls: usize,
    tool_results: usize,
    assistant_turns: usize,
    errors: usize,
    quality: isize,
}

struct TrainingGenerateArgs {
    cli: Option<String>,
    n: usize,
    timeout: Duration,
    delay: Duration,
    staging: PathBuf,
    dry_run: bool,
}

struct TrainingPrompt {
    prompt: String,
    component: String,
}

struct ValidationResult {
    passed: bool,
    score: usize,
    max_score: usize,
    results: Vec<(String, bool, String)>,
}

#[derive(Clone)]
struct BrowserMode {
    view: BrowserView,
    back_view: Option<BrowserView>,
    title: String,
    subtitle: String,
    footer: String,
    items: Vec<BrowserItem>,
    selected: usize,
    filter: String,
    filtering: bool,
}

#[derive(Clone)]
enum BrowserView {
    Skills,
    ModelPicker,
    StoragePicker,
    StorageDevices,
    McpManager,
    MarketTop,
    MarketCategory(String),
    MarketAll,
    MarketSearch(String),
}

#[derive(Clone)]
struct BrowserItem {
    label: String,
    description: String,
    meta: String,
    style: BrowserItemStyle,
    selectable: bool,
    action: BrowserAction,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BrowserItemStyle {
    Normal,
    Connected,
    Installed,
    Separator,
}

#[derive(Clone)]
enum BrowserAction {
    None,
    SkillActivate(String),
    ModelSwitch(String),
    ModelStorage,
    StorageSetPath(String),
    StorageOpenDevices,
    McpToggle(String),
    McpMarket,
    McpConnectAll,
    McpDisconnectAll,
    MarketToggle(String),
    MarketOpenCategory(String),
    MarketInstall(String),
}

struct InstallPromptMode {
    server_name: String,
    command: String,
    description: String,
    resolved_args: Vec<Option<String>>,
    required_env_vars: Vec<String>,
    env: Map<String, Value>,
    steps: Vec<InstallPromptStep>,
    step_index: usize,
}

enum InstallPromptStep {
    Placeholder { name: String, arg_index: usize },
    EnvVar(String),
}

struct SkillCatalogEntry {
    key: &'static str,
    name: &'static str,
    description: &'static str,
    category: &'static str,
    prompt: &'static str,
}

struct MarketCatalogEntry {
    name: &'static str,
    command: &'static str,
    args: &'static [&'static str],
    description: &'static str,
    category: &'static str,
    env_vars: &'static [&'static str],
}

impl UiState {
    fn new(
        model: String,
        cwd: PathBuf,
        config: AppConfig,
        session_id: String,
        prior_messages: Vec<ChatMessage>,
    ) -> Self {
        Self {
            output_lines: if prior_messages.is_empty() {
                welcome_lines()
            } else {
                render_messages(&prior_messages)
            },
            scroll_offset: 0,
            auto_scroll: true,
            processing: false,
            plan_mode: false,
            model,
            session_id,
            cwd,
            config,
            completion_index: 0,
            pending_confirm: None,
            pending_diff: None,
            input_history: load_input_history(),
            history_index: None,
            history_draft: String::new(),
            pending_watch_prompts: Vec::new(),
            watchers: BTreeMap::new(),
            background_tasks: Vec::new(),
            next_background_task_id: 1,
            test_cmd: String::new(),
            constraints_mode: ConstraintsMode {
                active: false,
                adding: false,
                editing: false,
                edit_index: 0,
                selected: 0,
                rules: Vec::new(),
            },
            review_mode: ReviewMode {
                active: false,
                examples: Vec::new(),
                index: 0,
                staging_file: String::new(),
                approved_file: approved_training_data_path(),
                approved: 0,
                discarded: 0,
                skipped: 0,
            },
            browser_mode: None,
            install_prompt: None,
            project_context: String::new(),
        }
    }

    fn append(&mut self, text: impl Into<String>) {
        self.output_lines
            .extend(text.into().lines().map(str::to_owned));
        if self.auto_scroll {
            self.scroll_offset = 0;
        }
    }

    fn reset_output(&mut self) {
        self.output_lines = welcome_lines();
        self.scroll_offset = 0;
        self.auto_scroll = true;
    }

    fn stop_all_watchers(&mut self) -> usize {
        let count = self.watchers.len();
        for handle in self.watchers.values() {
            handle.active.store(false, Ordering::Relaxed);
        }
        self.watchers.clear();
        count
    }

    fn apply_project_config(&mut self) {
        let config = load_project_vibn(&self.cwd);
        self.project_context = render_project_context(&config);
        if let Some(model) = config.get("model").and_then(Value::as_str) {
            self.model = model.to_owned();
        }
        if let Some(test_cmd) = config.get("test_cmd").and_then(Value::as_str) {
            self.test_cmd = test_cmd.to_owned();
        }
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter(stdout: &mut Stdout) -> io::Result<Self> {
        if !io::stdin().is_terminal() || !stdout.is_terminal() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Vibn TUI requires an interactive terminal; pass a prompt for non-interactive use",
            ));
        }
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
    }
}

pub fn run_tui(
    config: AppConfig,
    model: String,
    session_id: String,
    cwd: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = io::stdout();
    let _guard = TerminalGuard::enter(&mut stdout)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut input = build_input();
    let (tx, rx) = mpsc::channel();
    let (_, prior_messages) = load_transcript(&session_id)?;
    let mut state = UiState::new(model, cwd, config, session_id, prior_messages);
    let mut hook_context = Map::new();
    hook_context.insert("model".to_owned(), Value::String(state.model.clone()));
    hook_context.insert(
        "cwd".to_owned(),
        Value::String(state.cwd.display().to_string()),
    );
    run_hooks(&state.config, HOOK_SESSION_START, Some(&hook_context));
    state.apply_project_config();
    if let Err(error) = sync_mcp_servers_from_config(&state.config) {
        state.append(format!("MCP auto-connect failed: {error}"));
    }

    loop {
        terminal.draw(|frame| render(frame, &state, &input))?;

        while let Ok(event) = rx.try_recv() {
            match event {
                UiEvent::Append(text) => state.append(text),
                UiEvent::SetProcessing(processing) => state.processing = processing,
                UiEvent::BackgroundFinished {
                    id,
                    status,
                    summary,
                } => {
                    if let Some(task) = state.background_tasks.iter_mut().find(|task| task.id == id)
                    {
                        task.status = status;
                        task.result_summary = summary.clone();
                    }
                    let icon = if status == "done" { "✓" } else { "✗" };
                    state.append(format!(
                        "Background #{id} {icon}  {}",
                        summary.chars().take(150).collect::<String>()
                    ));
                }
                UiEvent::WatchTriggered { prompt, changed } => {
                    state.pending_watch_prompts.push((prompt, changed));
                }
                UiEvent::RequestConfirm {
                    tool,
                    cmd,
                    reason,
                    reply,
                } => {
                    state.pending_confirm = Some(PendingConfirm {
                        tool,
                        cmd,
                        reason,
                        reply,
                    });
                }
                UiEvent::RequestDiff {
                    path,
                    diff_lines,
                    reply,
                } => {
                    state.pending_diff = Some(PendingDiff {
                        path,
                        diff_lines,
                        reply,
                    });
                }
                UiEvent::ClearConfirm => state.pending_confirm = None,
                UiEvent::ClearDiff => state.pending_diff = None,
            }
        }

        if !state.processing
            && state.pending_confirm.is_none()
            && state.pending_diff.is_none()
            && !state.pending_watch_prompts.is_empty()
        {
            let (prompt, changed) = state.pending_watch_prompts.remove(0);
            state.append(format!("Watch: {changed} changed — running agent…"));
            state.append(format!("> {prompt}"));
            state.processing = true;
            spawn_agent_task(
                expand_file_mentions(&prompt, &state.cwd),
                state.plan_mode,
                state.project_context.clone(),
                state.model.clone(),
                state.session_id.clone(),
                state.cwd.clone(),
                state.config.clone(),
                tx.clone(),
            );
        }

        if !event::poll(Duration::from_millis(50))? {
            continue;
        }

        match event::read()? {
            Event::Key(key) if should_quit(key) => break,
            Event::Key(key) => {
                if matches!(
                    handle_key_event(key, &mut input, &mut state, &tx)?,
                    InputAction::Quit
                ) {
                    break;
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => {
                    state.auto_scroll = false;
                    state.scroll_offset = state.scroll_offset.saturating_add(2);
                }
                MouseEventKind::ScrollDown => {
                    state.scroll_offset = state.scroll_offset.saturating_sub(2);
                    if state.scroll_offset == 0 {
                        state.auto_scroll = true;
                    }
                }
                _ => {}
            },
            Event::Resize(_, _) => {}
            _ => {}
        }
    }

    state.stop_all_watchers();
    Ok(())
}

fn build_input() -> TextArea<'static> {
    let mut input = TextArea::default();
    input.set_block(Block::default().borders(Borders::NONE));
    input.set_cursor_line_style(Style::default());
    input
}

fn welcome_lines() -> Vec<String> {
    vec![
        "Vibn — local AI coding agent".to_owned(),
        String::new(),
        "Ask for code edits, debugging, refactors, tests, explanations, or repo exploration."
            .to_owned(),
        "Vibn can read files, search code, edit with diff review, run commands, and remember project facts."
            .to_owned(),
        String::new(),
        "Start here:".to_owned(),
        "  /help        show commands".to_owned(),
        "  /model       switch local model".to_owned(),
        "  /mcp         manage MCP servers".to_owned(),
        "  /skills      activate focused agent modes".to_owned(),
        "  /memory      show remembered project facts".to_owned(),
        String::new(),
        "Shortcuts: Enter sends  •  Alt+Enter or Shift+Enter inserts newline  •  Ctrl+C exits"
            .to_owned(),
    ]
}

fn render_messages(messages: &[ChatMessage]) -> Vec<String> {
    let mut lines = Vec::new();
    for message in messages {
        match message.role.as_str() {
            "system" => {}
            "user" => lines.push(format!("> {}", message.content)),
            "assistant" => lines.extend(message.content.lines().map(str::to_owned)),
            "tool" => {
                let label = message.name.as_deref().unwrap_or("tool");
                lines.push(format!("[tool:{label}] {}", message.content));
            }
            _ => lines.extend(message.content.lines().map(str::to_owned)),
        }
    }
    if lines.is_empty() {
        welcome_lines()
    } else {
        lines
    }
}

fn should_quit(key: KeyEvent) -> bool {
    matches!(key, KeyEvent { code: KeyCode::Char('c'), modifiers, .. } if modifiers.contains(KeyModifiers::CONTROL))
}

fn handle_key_event(
    key: KeyEvent,
    input: &mut TextArea<'static>,
    state: &mut UiState,
    tx: &Sender<UiEvent>,
) -> Result<InputAction, Box<dyn std::error::Error>> {
    if state.review_mode.active {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Err(error) = review_action(state, "approve") {
                    state.append(format!("Error: {error}"));
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                if let Err(error) = review_action(state, "discard") {
                    state.append(format!("Error: {error}"));
                }
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                if let Err(error) = review_action(state, "skip") {
                    state.append(format!("Error: {error}"));
                }
            }
            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                if let Err(error) = review_action(state, "quit") {
                    state.append(format!("Error: {error}"));
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if let Err(error) = edit_current_review_example(state) {
                    state.append(format!("Error: {error}"));
                }
            }
            _ => {}
        }
        *input = build_input();
        return Ok(InputAction::Continue);
    }

    if let Some(install) = state.install_prompt.as_mut() {
        match key.code {
            KeyCode::Enter => {
                let value = current_input_text(input).trim().to_owned();
                if value.is_empty() {
                    state.install_prompt = None;
                    *input = build_input();
                    return Ok(InputAction::Continue);
                }
                if let Some(step) = install.steps.get(install.step_index) {
                    match step {
                        InstallPromptStep::Placeholder { arg_index, .. } => {
                            install.resolved_args[*arg_index] = Some(value);
                        }
                        InstallPromptStep::EnvVar(name) => {
                            install.env.insert(name.clone(), Value::String(value));
                        }
                    }
                }
                install.step_index += 1;
                *input = build_input();
                if install.step_index >= install.steps.len() {
                    match finish_install_prompt(state) {
                        Ok(message) if !message.is_empty() => state.append(message),
                        Ok(_) => {}
                        Err(error) => state.append(format!("Error: {error}")),
                    }
                }
                return Ok(InputAction::Continue);
            }
            KeyCode::Esc => {
                state.install_prompt = None;
                *input = build_input();
                return Ok(InputAction::Continue);
            }
            _ => {
                input.input(key);
                return Ok(InputAction::Continue);
            }
        }
    }

    if let Some(browser) = state.browser_mode.as_mut() {
        if browser.filtering {
            match key.code {
                KeyCode::Enter => {
                    browser.filter = current_input_text(input).trim().to_owned();
                    browser.filtering = false;
                    *input = build_input();
                    refresh_browser_mode(state);
                    return Ok(InputAction::Continue);
                }
                KeyCode::Esc => {
                    browser.filtering = false;
                    *input = build_input();
                    return Ok(InputAction::Continue);
                }
                _ => {
                    input.input(key);
                    return Ok(InputAction::Continue);
                }
            }
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(index) = next_selectable_index(&browser.items, browser.selected, -1) {
                    browser.selected = index;
                }
                return Ok(InputAction::Continue);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(index) = next_selectable_index(&browser.items, browser.selected, 1) {
                    browser.selected = index;
                }
                return Ok(InputAction::Continue);
            }
            KeyCode::Char('d') => {
                if matches!(browser.view, BrowserView::ModelPicker) {
                    let selected = browser.items.get(browser.selected).cloned();
                    if let Some(item) = selected {
                        if let BrowserAction::ModelSwitch(model_name) = item.action {
                            if model_name == state.model {
                                state.append(format!(
                                    "Cannot delete active model {}. Switch models first.",
                                    model_name
                                ));
                            } else if !list_installed_models(&state.config).contains(&model_name) {
                                state.append(format!("{model_name} is not installed."));
                            } else {
                                match delete_installed_model(&state.config, &model_name) {
                                    Ok(()) => state.append(format!("Deleted {model_name}.")),
                                    Err(error) => state.append(format!("Error: {error}")),
                                }
                                refresh_browser_mode(state);
                            }
                        }
                    }
                }
                return Ok(InputAction::Continue);
            }
            KeyCode::Char('/') => {
                browser.filtering = true;
                set_input_text(input, &browser.filter);
                return Ok(InputAction::Continue);
            }
            KeyCode::Enter => {
                let action = browser
                    .items
                    .get(browser.selected)
                    .map(|item| item.action.clone())
                    .unwrap_or(BrowserAction::None);
                match action {
                    BrowserAction::None => {}
                    BrowserAction::SkillActivate(key) => {
                        close_browser_mode(state, input);
                        match activate_skill_into_session(state, &key) {
                            Ok(message) => state.append(message),
                            Err(error) => state.append(format!("Error: {error}")),
                        }
                    }
                    BrowserAction::ModelSwitch(model_name) => {
                        close_browser_mode(state, input);
                        match switch_model(state, &model_name) {
                            Ok(message) => state.append(message),
                            Err(error) => state.append(format!("Error: {error}")),
                        }
                    }
                    BrowserAction::ModelStorage => {
                        let back_view = state.browser_mode.as_ref().map(|mode| mode.view.clone());
                        open_browser_mode(state, input, BrowserView::StoragePicker, back_view);
                    }
                    BrowserAction::StorageSetPath(path) => {
                        let cwd = state.cwd.clone();
                        match apply_model_path_change(&mut state.config, &cwd, &path) {
                            Ok(message) => state.append(message),
                            Err(error) => state.append(format!("Error: {error}")),
                        }
                        let target_view = state
                            .browser_mode
                            .as_ref()
                            .and_then(|mode| mode.back_view.clone())
                            .unwrap_or(BrowserView::ModelPicker);
                        open_browser_mode(state, input, target_view, None);
                    }
                    BrowserAction::StorageOpenDevices => {
                        let back_view = state.browser_mode.as_ref().map(|mode| mode.view.clone());
                        open_browser_mode(state, input, BrowserView::StorageDevices, back_view);
                    }
                    BrowserAction::McpToggle(name) => {
                        let result = if list_connected_mcp_servers()
                            .unwrap_or_default()
                            .iter()
                            .any(|status| status.name == name)
                        {
                            disconnect_mcp_server(&name).map(|_| format!("Disconnected {name}"))
                        } else {
                            let servers = configured_mcp_servers(&state.config);
                            let server = servers
                                .get(&name)
                                .and_then(Value::as_object)
                                .cloned()
                                .ok_or_else(|| format!("Unknown MCP server: {name}"));
                            match server {
                                Ok(server) => {
                                    let command = server
                                        .get("command")
                                        .and_then(Value::as_str)
                                        .ok_or_else(|| format!("Missing command for {name}"));
                                    match command {
                                        Ok(command) => {
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
                                                .and_then(Value::as_object)
                                                .cloned()
                                                .unwrap_or_default();
                                            connect_mcp_server(&name, command, &args, &env_vars)
                                                .map(|tool_count| {
                                                    format!(
                                                        "Connected {} ({} tool{})",
                                                        name,
                                                        tool_count,
                                                        if tool_count == 1 { "" } else { "s" }
                                                    )
                                                })
                                        }
                                        Err(error) => Err(error),
                                    }
                                }
                                Err(error) => Err(error),
                            }
                        };
                        match result {
                            Ok(message) => state.append(message),
                            Err(error) => state.append(format!("Error: {error}")),
                        }
                        refresh_browser_mode(state);
                    }
                    BrowserAction::McpMarket => {
                        let back_view = state.browser_mode.as_ref().map(|mode| mode.view.clone());
                        open_browser_mode(state, input, BrowserView::MarketTop, back_view);
                    }
                    BrowserAction::McpConnectAll => {
                        let servers = configured_mcp_servers(&state.config);
                        let mut results = Vec::new();
                        let mut names = servers.keys().cloned().collect::<Vec<_>>();
                        names.sort();
                        for name in names {
                            if list_connected_mcp_servers()
                                .unwrap_or_default()
                                .iter()
                                .any(|status| status.name == name)
                            {
                                continue;
                            }
                            let Some(server) = servers.get(&name).and_then(Value::as_object) else {
                                continue;
                            };
                            let Some(command) = server.get("command").and_then(Value::as_str)
                            else {
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
                                .and_then(Value::as_object)
                                .cloned()
                                .unwrap_or_default();
                            match connect_mcp_server(&name, command, &args, &env_vars) {
                                Ok(tool_count) => results.push(format!(
                                    "Connected {} ({} tool{})",
                                    name,
                                    tool_count,
                                    if tool_count == 1 { "" } else { "s" }
                                )),
                                Err(error) => results.push(format!("{}: {}", name, error)),
                            }
                        }
                        if !results.is_empty() {
                            state.append(results.join("\n"));
                        }
                        refresh_browser_mode(state);
                    }
                    BrowserAction::McpDisconnectAll => {
                        let connected = list_connected_mcp_servers().unwrap_or_default();
                        let mut results = Vec::new();
                        for status in connected {
                            match disconnect_mcp_server(&status.name) {
                                Ok(()) => results.push(format!("Disconnected {}", status.name)),
                                Err(error) => results.push(format!("{}: {}", status.name, error)),
                            }
                        }
                        if !results.is_empty() {
                            state.append(results.join("\n"));
                        }
                        refresh_browser_mode(state);
                    }
                    BrowserAction::MarketToggle(name) => {
                        let result = if list_connected_mcp_servers()
                            .unwrap_or_default()
                            .iter()
                            .any(|status| status.name == name)
                        {
                            disconnect_mcp_server(&name).map(|_| format!("Disconnected {name}"))
                        } else {
                            let servers = configured_mcp_servers(&state.config);
                            let server = servers
                                .get(&name)
                                .and_then(Value::as_object)
                                .cloned()
                                .ok_or_else(|| format!("Unknown MCP server: {name}"));
                            match server {
                                Ok(server) => {
                                    let command = server
                                        .get("command")
                                        .and_then(Value::as_str)
                                        .ok_or_else(|| format!("Missing command for {name}"));
                                    match command {
                                        Ok(command) => {
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
                                                .and_then(Value::as_object)
                                                .cloned()
                                                .unwrap_or_default();
                                            connect_mcp_server(&name, command, &args, &env_vars)
                                                .map(|tool_count| {
                                                    format!(
                                                        "Connected {} ({} tool{})",
                                                        name,
                                                        tool_count,
                                                        if tool_count == 1 { "" } else { "s" }
                                                    )
                                                })
                                        }
                                        Err(error) => Err(error),
                                    }
                                }
                                Err(error) => Err(error),
                            }
                        };
                        match result {
                            Ok(message) => state.append(message),
                            Err(error) => state.append(format!("Error: {error}")),
                        }
                        refresh_browser_mode(state);
                    }
                    BrowserAction::MarketOpenCategory(category) => {
                        let back_view = state.browser_mode.as_ref().map(|mode| mode.view.clone());
                        open_browser_mode(
                            state,
                            input,
                            BrowserView::MarketCategory(category),
                            back_view,
                        );
                    }
                    BrowserAction::MarketInstall(name) => {
                        match start_marketplace_install(state, input, &name) {
                            Ok(message) if !message.is_empty() => state.append(message),
                            Ok(_) => {}
                            Err(error) => state.append(format!("Error: {error}")),
                        }
                    }
                }
                return Ok(InputAction::Continue);
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                let back_view = browser.back_view.clone();
                if let Some(view) = back_view {
                    open_browser_mode(state, input, view, None);
                } else {
                    close_browser_mode(state, input);
                }
                return Ok(InputAction::Continue);
            }
            _ => return Ok(InputAction::Continue),
        }
    }

    if let Some(action) = pending_modal_response(key, state) {
        return Ok(action);
    }

    if state.constraints_mode.active {
        if state.constraints_mode.adding || state.constraints_mode.editing {
            match key.code {
                KeyCode::Enter => {
                    let trimmed = current_input_text(input).trim().to_owned();
                    if !trimmed.is_empty() {
                        if state.constraints_mode.editing {
                            state.constraints_mode.rules[state.constraints_mode.edit_index] =
                                trimmed;
                            state.constraints_mode.selected = state.constraints_mode.edit_index;
                        } else {
                            state.constraints_mode.rules.push(trimmed);
                            state.constraints_mode.selected =
                                state.constraints_mode.rules.len().saturating_sub(1);
                        }
                        save_constraints(&state.constraints_mode.rules)?;
                    }
                    state.constraints_mode.adding = false;
                    state.constraints_mode.editing = false;
                    *input = build_input();
                    return Ok(InputAction::Continue);
                }
                KeyCode::Esc => {
                    state.constraints_mode.adding = false;
                    state.constraints_mode.editing = false;
                    *input = build_input();
                    return Ok(InputAction::Continue);
                }
                _ => {
                    input.input(key);
                    return Ok(InputAction::Continue);
                }
            }
        }

        match key.code {
            KeyCode::Up => {
                if state.constraints_mode.selected > 0 {
                    state.constraints_mode.selected -= 1;
                }
                return Ok(InputAction::Continue);
            }
            KeyCode::Down => {
                if !state.constraints_mode.rules.is_empty() {
                    state.constraints_mode.selected = (state.constraints_mode.selected + 1)
                        .min(state.constraints_mode.rules.len() - 1);
                }
                return Ok(InputAction::Continue);
            }
            KeyCode::Char('d') => {
                if !state.constraints_mode.rules.is_empty() {
                    state
                        .constraints_mode
                        .rules
                        .remove(state.constraints_mode.selected);
                    state.constraints_mode.selected = state
                        .constraints_mode
                        .selected
                        .min(state.constraints_mode.rules.len().saturating_sub(1));
                    save_constraints(&state.constraints_mode.rules)?;
                }
                return Ok(InputAction::Continue);
            }
            KeyCode::Char('a') => {
                state.constraints_mode.adding = true;
                *input = build_input();
                return Ok(InputAction::Continue);
            }
            KeyCode::Char('e') => {
                if let Some(rule) = state
                    .constraints_mode
                    .rules
                    .get(state.constraints_mode.selected)
                {
                    state.constraints_mode.editing = true;
                    state.constraints_mode.edit_index = state.constraints_mode.selected;
                    set_input_text(input, rule);
                }
                return Ok(InputAction::Continue);
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                state.constraints_mode.active = false;
                state.constraints_mode.adding = false;
                state.constraints_mode.editing = false;
                *input = build_input();
                return Ok(InputAction::Continue);
            }
            _ => return Ok(InputAction::Continue),
        }
    }

    let text_before = current_input_text(input);
    let slash_before = slash_context(&text_before, &state.config, &state.cwd);

    match key.code {
        KeyCode::Enter
            if key.modifiers.contains(KeyModifiers::ALT)
                || key.modifiers.contains(KeyModifiers::SHIFT) =>
        {
            input.insert_newline();
        }
        KeyCode::Tab => {
            if let Some(context) = slash_before.as_ref() {
                if let Some(choice) = selected_completion(context, state.completion_index) {
                    set_input_text(input, &choice);
                    sync_completion_index(state, input);
                    return Ok(InputAction::Continue);
                }
            }
            input.input(key);
            state.history_index = None;
        }
        KeyCode::Up => {
            if let Some(context) = slash_before.as_ref() {
                if !context.items.is_empty() {
                    if state.completion_index == 0 {
                        state.completion_index = context.items.len() - 1;
                    } else {
                        state.completion_index -= 1;
                    }
                    return Ok(InputAction::Continue);
                }
            }
            input.input(key);
            state.history_index = None;
        }
        KeyCode::Down => {
            if let Some(context) = slash_before.as_ref() {
                if !context.items.is_empty() {
                    state.completion_index = (state.completion_index + 1) % context.items.len();
                    return Ok(InputAction::Continue);
                }
            }
            input.input(key);
            state.history_index = None;
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            history_backward(input, state);
            return Ok(InputAction::Continue);
        }
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            history_forward(input, state);
            return Ok(InputAction::Continue);
        }
        KeyCode::Enter => {
            let text = current_input_text(input);
            let mut trimmed = text.trim().to_owned();
            if trimmed.is_empty() {
                *input = build_input();
                sync_completion_index(state, input);
                return Ok(InputAction::Continue);
            }
            let exact_slash_command = trimmed.starts_with('/')
                && !trimmed.contains('\n')
                && exact_command(&trimmed).is_some();
            if let Some(context) = slash_before.as_ref() {
                if !context.items.is_empty() && trimmed.starts_with('/') && !trimmed.contains('\n')
                    && !exact_slash_command
                {
                    if let Some(choice) = selected_completion(context, state.completion_index) {
                        if choice.ends_with(' ') {
                            set_input_text(input, &choice);
                            sync_completion_index(state, input);
                            return Ok(InputAction::Continue);
                        }
                        trimmed = choice;
                    }
                }
            }
            *input = build_input();
            remember_history_entry(state, &trimmed);
            if trimmed.starts_with('/') && !trimmed.contains('\n') {
                return execute_slash_command(input, state, &trimmed, tx.clone());
            }
            if trimmed.starts_with('!') && !trimmed.contains('\n') {
                let shell_cmd = trimmed[1..].trim().to_owned();
                if shell_cmd.is_empty() {
                    state.append("Usage: !<command>  (e.g. !ls, !git status, !npm test)");
                    return Ok(InputAction::Continue);
                }
                if state.processing {
                    state.append("Still processing the previous request.");
                    return Ok(InputAction::Continue);
                }
                state.append(format!("!{shell_cmd}"));
                state.processing = true;
                spawn_shell_task(
                    shell_cmd,
                    state.cwd.clone(),
                    state.config.clone(),
                    tx.clone(),
                );
                return Ok(InputAction::Continue);
            }
            if state.processing {
                state.append("Still processing the previous request.");
                return Ok(InputAction::Continue);
            }
            state.append(format!("> {trimmed}"));
            state.processing = true;
            let expanded = expand_file_mentions(&trimmed, &state.cwd);
            spawn_agent_task(
                expanded,
                state.plan_mode,
                state.project_context.clone(),
                state.model.clone(),
                state.session_id.clone(),
                state.cwd.clone(),
                state.config.clone(),
                tx.clone(),
            );
        }
        KeyCode::PageUp => {
            state.auto_scroll = false;
            state.scroll_offset = state.scroll_offset.saturating_add(10);
        }
        KeyCode::PageDown => {
            state.scroll_offset = state.scroll_offset.saturating_sub(10);
            if state.scroll_offset == 0 {
                state.auto_scroll = true;
            }
        }
        KeyCode::Esc => return Ok(InputAction::Quit),
        _ => {
            input.input(key);
            state.history_index = None;
        }
    }

    sync_completion_index(state, input);
    Ok(InputAction::Continue)
}

fn load_input_history() -> Vec<String> {
    let path = history_file_path();
    fs::read_to_string(path)
        .map(|content| {
            content
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn history_file_path() -> PathBuf {
    vibn_config_file()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("history")
}

fn pins_file_path() -> PathBuf {
    vibn_config_file()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("pins.json")
}

fn load_pins_from(path: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    Ok(value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .map(str::to_owned)
        .collect())
}

fn load_pins() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    load_pins_from(&pins_file_path())
}

fn append_pin(note: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = pins_file_path();
    let mut pins = if path.exists() {
        serde_json::from_str::<Value>(&fs::read_to_string(&path)?)?
            .as_array()
            .cloned()
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    pins.push(json!({ "text": note }));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(&pins)?)?;
    Ok(())
}

fn last_agent_block(lines: &[String]) -> Option<String> {
    let mut block = Vec::new();
    let mut started = false;
    for line in lines.iter().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if started {
                break;
            }
            continue;
        }
        if trimmed.starts_with("> ") || trimmed.starts_with('!') || trimmed.starts_with("[tool:") {
            if started {
                break;
            }
            continue;
        }
        started = true;
        block.push(line.clone());
    }
    if block.is_empty() {
        None
    } else {
        block.reverse();
        Some(block.join("\n"))
    }
}

fn copy_to_clipboard(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;

    if let Ok(mut child) = Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(text.as_bytes())?;
        }
        if child.wait()?.success() {
            return Ok(());
        }
    }

    let mut child = Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes())?;
    }
    if child.wait()?.success() {
        Ok(())
    } else {
        Err("clipboard command failed".into())
    }
}

fn with_terminal_suspended<T, F>(action: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnOnce() -> Result<T, Box<dyn std::error::Error>>,
{
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    let result = action();
    let restore = (|| -> io::Result<()> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        Ok(())
    })();
    match (result, restore) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (_, Err(error)) => Err(error.into()),
    }
}

fn open_in_editor(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let editor = env::var("EDITOR").unwrap_or_else(|_| "nano".to_owned());
    with_terminal_suspended(|| {
        let status = Command::new(&editor).arg(path).status()?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("editor exited with status {status}").into())
        }
    })
}

fn run_git_capture(
    cwd: &Path,
    args: &[&str],
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    Ok(Command::new("git").args(args).current_dir(cwd).output()?)
}

fn command_output_text(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if stdout.is_empty() { stderr } else { stdout }
}

fn normalize_commit_message(text: &str) -> String {
    text.trim().trim_matches('`').trim().to_owned()
}

fn temp_commit_message_path() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir().join(format!("vibn-commit-message-{unique}.txt"))
}

fn temp_review_edit_path() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir().join(format!("vibn-review-edit-{unique}.tsx"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn split_shell_words(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match quote {
            Some(active) if ch == active => quote = None,
            Some(_) => current.push(ch),
            None if matches!(ch, '"' | '\'') => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            None if ch == '\\' => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            None => current.push(ch),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

const SKILL_CATEGORIES: &[(&str, &str)] = &[
    ("analysis", "Analysis & Review"),
    ("editing", "Code Generation & Editing"),
    ("git", "Git & Version Control"),
];

const SKILLS: &[SkillCatalogEntry] = &[
    SkillCatalogEntry {
        key: "code-review",
        name: "Code Review",
        description: "Review code for bugs, security issues, performance, and best practices",
        category: "analysis",
        prompt: "You are now in CODE REVIEW mode. Analyze the code thoroughly for:\n1. **Bugs & Logic Errors** — off-by-one, null refs, race conditions, edge cases\n2. **Security** — injection, XSS, auth issues, secrets in code, OWASP top 10\n3. **Performance** — N+1 queries, unnecessary allocations, missing indexes\n4. **Readability** — naming, complexity, dead code, unclear logic\n5. **Best Practices** — error handling, testing, typing, idiomatic patterns\n\nFor each issue found, state: file:line, severity (critical/warning/info), what's wrong, and a fix.\nStart by reading the relevant files, then give your review.",
    },
    SkillCatalogEntry {
        key: "refactor",
        name: "Refactor",
        description: "Refactor code: extract functions, reduce duplication, simplify logic",
        category: "editing",
        prompt: "You are now in REFACTOR mode. Your goal is to improve code structure without changing behavior:\n1. Read the target code first to understand it fully\n2. Identify: duplicated logic, long functions, deep nesting, unclear names\n3. Plan refactoring steps (explain before doing)\n4. Make changes incrementally using edit_file\n5. Run tests after each significant change to verify behavior is preserved\n6. Summarize what you changed and why",
    },
    SkillCatalogEntry {
        key: "debug",
        name: "Debug",
        description: "Systematic debugging: reproduce, diagnose, fix, verify",
        category: "analysis",
        prompt: "You are now in DEBUG mode. Follow a systematic approach:\n1. **Understand** — What's the expected vs actual behavior?\n2. **Reproduce** — Run the code to see the error firsthand\n3. **Locate** — Read relevant files, search for error messages, check logs\n4. **Diagnose** — Form a hypothesis about the root cause\n5. **Fix** — Make the minimal change to fix the issue\n6. **Verify** — Run the code/tests again to confirm the fix\n7. **Explain** — What was wrong and why your fix works",
    },
    SkillCatalogEntry {
        key: "test",
        name: "Write Tests",
        description: "Write comprehensive tests for existing code",
        category: "editing",
        prompt: "You are now in TEST WRITING mode:\n1. Read the code to understand what needs testing\n2. Identify the testing framework already in use (or suggest one)\n3. Write tests covering: happy path, edge cases, error cases, boundary conditions\n4. Follow existing test patterns and conventions in the project\n5. Run the tests to verify they pass\n6. Aim for meaningful coverage, not 100% line coverage",
    },
    SkillCatalogEntry {
        key: "explain",
        name: "Explain Code",
        description: "Explain how code works in plain language",
        category: "analysis",
        prompt: "You are now in EXPLAIN mode. Help the user understand code:\n1. Read the target code\n2. Explain what it does at a high level (the \"what\")\n3. Walk through the key logic step by step (the \"how\")\n4. Explain design decisions and patterns used (the \"why\")\n5. Note any non-obvious behavior, gotchas, or clever tricks\n6. Use analogies when helpful. Adjust detail level to the user's question.",
    },
    SkillCatalogEntry {
        key: "scaffold",
        name: "Scaffold Project",
        description: "Generate boilerplate: new project, feature, component, API route",
        category: "editing",
        prompt: "You are now in SCAFFOLD mode. Generate well-structured boilerplate code:\n1. Ask what the user wants to scaffold (or infer from context)\n2. Follow the project's existing patterns, conventions, and tech stack\n3. Create all necessary files: source, tests, configs, types\n4. Include TODO comments for parts the user needs to customize\n5. Run any necessary setup commands (npm init, pip install, etc.)\n6. Show the user what was created and what to do next",
    },
    SkillCatalogEntry {
        key: "git-commit",
        name: "Smart Commit",
        description: "Stage changes and create a well-crafted commit message",
        category: "git",
        prompt: "You are now in COMMIT mode:\n1. Run `git status` and `git diff` to see all changes\n2. Analyze what was changed and why\n3. Stage the appropriate files (be selective, don't stage unrelated changes)\n4. Write a concise, descriptive commit message following conventional commits:\n   - feat: new feature\n   - fix: bug fix\n   - refactor: code restructuring\n   - docs: documentation\n   - test: adding tests\n   - chore: maintenance\n5. Show the commit message to the user and ask for confirmation before committing",
    },
    SkillCatalogEntry {
        key: "git-pr",
        name: "Create PR",
        description: "Create a pull request with a good title and description",
        category: "git",
        prompt: "You are now in PR mode:\n1. Run `git log main..HEAD` (or appropriate base branch) to see all commits\n2. Run `git diff main..HEAD` to see all changes\n3. Summarize: what changed, why, and how to test\n4. Create a PR using `gh pr create` with:\n   - Clear title (under 70 chars)\n   - Description with: Summary, Changes, Test Plan\n5. Show the PR URL when done",
    },
    SkillCatalogEntry {
        key: "security-audit",
        name: "Security Audit",
        description: "Scan code for security vulnerabilities and hardcoded secrets",
        category: "analysis",
        prompt: "You are now in SECURITY AUDIT mode. Check for:\n1. **Hardcoded Secrets** — API keys, passwords, tokens in source code\n2. **Injection** — SQL injection, command injection, XSS, path traversal\n3. **Auth Issues** — missing auth checks, weak session handling, CSRF\n4. **Dependencies** — known vulnerable packages (Cargo.toml, package manifests, lockfiles)\n5. **Config** — debug mode in prod, permissive CORS, missing rate limiting\n6. **Data** — PII exposure, missing encryption, insecure storage\n\nSearch through the codebase systematically. For each finding:\n- Severity: CRITICAL / HIGH / MEDIUM / LOW\n- Location: file:line\n- Issue: what's wrong\n- Fix: how to fix it",
    },
    SkillCatalogEntry {
        key: "optimize",
        name: "Optimize Performance",
        description: "Find and fix performance bottlenecks",
        category: "analysis",
        prompt: "You are now in OPTIMIZE mode. Look for performance issues:\n1. Read the code and identify hot paths\n2. Check for: N+1 queries, unnecessary re-renders, large bundle imports,\n   missing caching, synchronous I/O, memory leaks, inefficient algorithms\n3. Profile if possible (run benchmarks, check bundle size)\n4. Suggest optimizations ranked by impact\n5. Implement the highest-impact fixes\n6. Verify performance improved",
    },
    SkillCatalogEntry {
        key: "document",
        name: "Document Code",
        description: "Generate documentation: README, API docs, inline comments",
        category: "editing",
        prompt: "You are now in DOCUMENT mode:\n1. Read the code to understand the project/module\n2. Generate appropriate documentation:\n   - README.md for projects (setup, usage, architecture)\n   - JSDoc/docstrings for functions and classes\n   - API documentation for endpoints\n   - Architecture decision records for complex systems\n3. Follow existing documentation style in the project\n4. Be concise but thorough. Document the \"why\" not just the \"what\".",
    },
    SkillCatalogEntry {
        key: "migrate",
        name: "Migrate/Upgrade",
        description: "Help migrate between versions, frameworks, or languages",
        category: "editing",
        prompt: "You are now in MIGRATE mode:\n1. Understand the current state (versions, dependencies, patterns)\n2. Research what needs to change for the target version/framework\n3. Create a migration plan with ordered steps\n4. Execute each step, verifying as you go\n5. Update configs, dependencies, imports, and API calls\n6. Run tests after each major change\n7. Document any breaking changes or manual steps needed",
    },
];

const MARKET_CATEGORIES: &[(&str, &str)] = &[
    ("filesystem", "File Systems"),
    ("database", "Databases"),
    ("dev-tools", "Developer Tools"),
    ("api", "APIs & Services"),
    ("ai", "AI & ML"),
    ("cloud", "Cloud & Infrastructure"),
    ("search", "Search Engines"),
    ("browser", "Browser Automation"),
    ("productivity", "Productivity"),
    ("remote", "Remote Servers"),
];

const MARKETPLACE: &[MarketCatalogEntry] = &[
    MarketCatalogEntry {
        name: "filesystem",
        command: "npx",
        args: &["-y", "@modelcontextprotocol/server-filesystem", "{path}"],
        description: "Read/write access to local filesystem",
        category: "filesystem",
        env_vars: &[],
    },
    MarketCatalogEntry {
        name: "google-drive",
        command: "npx",
        args: &["-y", "@modelcontextprotocol/server-gdrive"],
        description: "Google Drive file access and search",
        category: "filesystem",
        env_vars: &["GDRIVE_CREDENTIALS"],
    },
    MarketCatalogEntry {
        name: "postgres",
        command: "npx",
        args: &[
            "-y",
            "@modelcontextprotocol/server-postgres",
            "{connection_string}",
        ],
        description: "Query and manage PostgreSQL databases",
        category: "database",
        env_vars: &[],
    },
    MarketCatalogEntry {
        name: "sqlite",
        command: "uvx",
        args: &["mcp-server-sqlite", "--db-path", "{db_path}"],
        description: "Query and manage SQLite databases",
        category: "database",
        env_vars: &[],
    },
    MarketCatalogEntry {
        name: "mysql",
        command: "npx",
        args: &["-y", "@benborla29/mcp-server-mysql"],
        description: "Query and manage MySQL databases",
        category: "database",
        env_vars: &["MYSQL_HOST", "MYSQL_USER", "MYSQL_PASSWORD"],
    },
    MarketCatalogEntry {
        name: "redis",
        command: "npx",
        args: &["-y", "@gongrzhe/server-redis-mcp"],
        description: "Redis key-value store operations",
        category: "database",
        env_vars: &["REDIS_URL"],
    },
    MarketCatalogEntry {
        name: "supabase",
        command: "npx",
        args: &["-y", "@supabase/mcp-server-supabase"],
        description: "Supabase database, auth, and storage",
        category: "database",
        env_vars: &["SUPABASE_URL", "SUPABASE_SERVICE_ROLE_KEY"],
    },
    MarketCatalogEntry {
        name: "neon",
        command: "npx",
        args: &["-y", "@neondatabase/mcp-server-neon"],
        description: "Neon serverless Postgres management",
        category: "database",
        env_vars: &["NEON_API_KEY"],
    },
    MarketCatalogEntry {
        name: "git",
        command: "uvx",
        args: &["mcp-server-git"],
        description: "Git repository operations (log, diff, branch, blame)",
        category: "dev-tools",
        env_vars: &[],
    },
    MarketCatalogEntry {
        name: "github",
        command: "npx",
        args: &["-y", "@modelcontextprotocol/server-github"],
        description: "GitHub API: repos, issues, PRs, actions, code search",
        category: "dev-tools",
        env_vars: &["GITHUB_TOKEN"],
    },
    MarketCatalogEntry {
        name: "gitlab",
        command: "npx",
        args: &["-y", "@modelcontextprotocol/server-gitlab"],
        description: "GitLab API: repos, merge requests, pipelines",
        category: "dev-tools",
        env_vars: &["GITLAB_TOKEN"],
    },
    MarketCatalogEntry {
        name: "linear",
        command: "npx",
        args: &["-y", "mcp-server-linear"],
        description: "Linear issue tracker integration",
        category: "dev-tools",
        env_vars: &["LINEAR_API_KEY"],
    },
    MarketCatalogEntry {
        name: "sentry",
        command: "uvx",
        args: &["mcp-server-sentry"],
        description: "Sentry error tracking and monitoring",
        category: "dev-tools",
        env_vars: &["SENTRY_AUTH_TOKEN"],
    },
    MarketCatalogEntry {
        name: "docker",
        command: "npx",
        args: &["-y", "mcp-server-docker"],
        description: "Docker container and image management",
        category: "dev-tools",
        env_vars: &[],
    },
    MarketCatalogEntry {
        name: "kubernetes",
        command: "npx",
        args: &["-y", "mcp-server-kubernetes"],
        description: "Kubernetes cluster and pod management",
        category: "dev-tools",
        env_vars: &[],
    },
    MarketCatalogEntry {
        name: "fetch",
        command: "uvx",
        args: &["mcp-server-fetch"],
        description: "HTTP fetch: retrieve and convert web content to markdown",
        category: "api",
        env_vars: &[],
    },
    MarketCatalogEntry {
        name: "stripe",
        command: "npx",
        args: &["-y", "@stripe/agent-toolkit", "--mcp"],
        description: "Stripe payments API: customers, charges, subscriptions",
        category: "api",
        env_vars: &["STRIPE_SECRET_KEY"],
    },
    MarketCatalogEntry {
        name: "notion",
        command: "npx",
        args: &["-y", "notion-mcp-server"],
        description: "Notion pages, databases, and blocks API",
        category: "api",
        env_vars: &["NOTION_API_KEY"],
    },
    MarketCatalogEntry {
        name: "slack",
        command: "npx",
        args: &["-y", "@modelcontextprotocol/server-slack"],
        description: "Slack messaging, channels, and workspace management",
        category: "api",
        env_vars: &["SLACK_BOT_TOKEN"],
    },
    MarketCatalogEntry {
        name: "sequential-thinking",
        command: "npx",
        args: &["-y", "@modelcontextprotocol/server-sequential-thinking"],
        description: "Structured step-by-step reasoning and chain of thought",
        category: "ai",
        env_vars: &[],
    },
    MarketCatalogEntry {
        name: "memory",
        command: "npx",
        args: &["-y", "@modelcontextprotocol/server-memory"],
        description: "Persistent memory via knowledge graph (entities & relations)",
        category: "ai",
        env_vars: &[],
    },
    MarketCatalogEntry {
        name: "everart",
        command: "npx",
        args: &["-y", "@modelcontextprotocol/server-everart"],
        description: "AI image generation via EverArt",
        category: "ai",
        env_vars: &["EVERART_API_KEY"],
    },
    MarketCatalogEntry {
        name: "cloudflare",
        command: "npx",
        args: &["-y", "@cloudflare/mcp-server-cloudflare"],
        description: "Cloudflare Workers, KV, D1, R2 management",
        category: "cloud",
        env_vars: &["CLOUDFLARE_API_TOKEN"],
    },
    MarketCatalogEntry {
        name: "vercel",
        command: "npx",
        args: &["-y", "vercel-mcp"],
        description: "Vercel deployments, projects, and logs",
        category: "cloud",
        env_vars: &["VERCEL_TOKEN"],
    },
    MarketCatalogEntry {
        name: "aws",
        command: "npx",
        args: &["-y", "mcp-server-aws"],
        description: "AWS services: EC2, Lambda, S3, CloudWatch",
        category: "cloud",
        env_vars: &["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"],
    },
    MarketCatalogEntry {
        name: "brave-search",
        command: "npx",
        args: &["-y", "@modelcontextprotocol/server-brave-search"],
        description: "Web search via Brave Search API",
        category: "search",
        env_vars: &["BRAVE_API_KEY"],
    },
    MarketCatalogEntry {
        name: "exa",
        command: "npx",
        args: &["-y", "exa-mcp-server"],
        description: "Exa neural search engine for web content",
        category: "search",
        env_vars: &["EXA_API_KEY"],
    },
    MarketCatalogEntry {
        name: "puppeteer",
        command: "npx",
        args: &["-y", "@modelcontextprotocol/server-puppeteer"],
        description: "Headless browser automation via Puppeteer",
        category: "browser",
        env_vars: &[],
    },
    MarketCatalogEntry {
        name: "playwright",
        command: "npx",
        args: &["-y", "@playwright/mcp", "--headless"],
        description: "Browser automation and testing via Playwright",
        category: "browser",
        env_vars: &[],
    },
    MarketCatalogEntry {
        name: "browserbase",
        command: "npx",
        args: &["-y", "@browserbasehq/mcp-server-browserbase"],
        description: "Cloud browser sessions via Browserbase",
        category: "browser",
        env_vars: &["BROWSERBASE_API_KEY"],
    },
    MarketCatalogEntry {
        name: "google-maps",
        command: "npx",
        args: &["-y", "@modelcontextprotocol/server-google-maps"],
        description: "Google Maps geocoding, directions, and places",
        category: "productivity",
        env_vars: &["GOOGLE_MAPS_API_KEY"],
    },
    MarketCatalogEntry {
        name: "todoist",
        command: "npx",
        args: &["-y", "todoist-mcp-server"],
        description: "Todoist task and project management",
        category: "productivity",
        env_vars: &["TODOIST_API_KEY"],
    },
    MarketCatalogEntry {
        name: "obsidian",
        command: "npx",
        args: &["-y", "obsidian-mcp"],
        description: "Obsidian vault note reading, writing, and search",
        category: "productivity",
        env_vars: &["OBSIDIAN_VAULT_PATH"],
    },
    MarketCatalogEntry {
        name: "mcp-remote",
        command: "npx",
        args: &["-y", "mcp-remote", "{server_url}"],
        description: "Connect to any remote MCP server via SSE/HTTP",
        category: "remote",
        env_vars: &[],
    },
];

fn prompts_file_path() -> PathBuf {
    vibn_config_file()
        .parent()
        .map(|parent| parent.join("prompts.json"))
        .unwrap_or_else(|| PathBuf::from(".vibn/prompts.json"))
}

fn vibn_home_dir() -> PathBuf {
    vibn_config_file()
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn approved_training_data_path() -> PathBuf {
    vibn_home_dir().join("vibn_training_data.jsonl")
}

fn staging_dir_path() -> PathBuf {
    vibn_config_file()
        .parent()
        .map(|parent| parent.join("staging"))
        .unwrap_or_else(|| PathBuf::from(".vibn/staging"))
}

fn constraints_file_path() -> PathBuf {
    vibn_config_file()
        .parent()
        .map(|parent| parent.join("constraints.json"))
        .unwrap_or_else(|| PathBuf::from(".vibn/constraints.json"))
}

pub(crate) fn load_project_vibn(directory: &Path) -> Map<String, Value> {
    let path = directory.join(".vibn");
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

pub(crate) fn render_project_context(config: &Map<String, Value>) -> String {
    let mut parts = Vec::new();
    if let Some(prompt) = config.get("system_prompt").and_then(Value::as_str) {
        let trimmed = prompt.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_owned());
        }
    }
    if let Some(constraints) = config.get("constraints").and_then(Value::as_array) {
        let rules = constraints
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|rule| !rule.is_empty())
            .map(|rule| format!("- {rule}"))
            .collect::<Vec<_>>();
        if !rules.is_empty() {
            parts.push(format!("Project constraints:\n{}", rules.join("\n")));
        }
    }
    parts.join("\n\n")
}

fn skill_category_label(category: &str) -> &str {
    SKILL_CATEGORIES
        .iter()
        .find(|(key, _)| *key == category)
        .map(|(_, label)| *label)
        .unwrap_or(category)
}

fn get_skill(key: &str) -> Option<&'static SkillCatalogEntry> {
    SKILLS.iter().find(|skill| skill.key == key)
}

fn search_skills(query: &str) -> Vec<&'static SkillCatalogEntry> {
    let query = query.to_lowercase();
    SKILLS
        .iter()
        .filter(|skill| {
            skill.key.to_lowercase().contains(&query)
                || skill.name.to_lowercase().contains(&query)
                || skill.description.to_lowercase().contains(&query)
        })
        .collect()
}

fn category_label(category: &str) -> &str {
    MARKET_CATEGORIES
        .iter()
        .find(|(key, _)| *key == category)
        .map(|(_, label)| *label)
        .unwrap_or(category)
}

fn get_market_entry(name: &str) -> Option<&'static MarketCatalogEntry> {
    MARKETPLACE.iter().find(|entry| entry.name == name)
}

fn search_marketplace(query: &str) -> Vec<&'static MarketCatalogEntry> {
    let query = query.to_lowercase();
    MARKETPLACE
        .iter()
        .filter(|entry| {
            entry.name.to_lowercase().contains(&query)
                || entry.description.to_lowercase().contains(&query)
                || entry.category.to_lowercase().contains(&query)
        })
        .collect()
}

fn market_entries_by_category(category: &str) -> Vec<&'static MarketCatalogEntry> {
    MARKETPLACE
        .iter()
        .filter(|entry| entry.category == category)
        .collect()
}

fn configured_mcp_servers(config: &AppConfig) -> Map<String, Value> {
    config
        .extra
        .get("mcp_servers")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn resolve_skill_matches(query: &str) -> Result<&'static SkillCatalogEntry, Vec<String>> {
    if let Some(skill) = get_skill(query) {
        return Ok(skill);
    }
    let matches = search_skills(query);
    if matches.len() == 1 {
        Ok(matches[0])
    } else {
        Err(matches.iter().map(|skill| skill.key.to_owned()).collect())
    }
}

fn resolve_marketplace_matches(
    query: &str,
) -> Result<&'static MarketCatalogEntry, Vec<&'static MarketCatalogEntry>> {
    if let Some(entry) = get_market_entry(query) {
        return Ok(entry);
    }
    let matches = search_marketplace(query);
    if matches.len() == 1 {
        Ok(matches[0])
    } else {
        Err(matches)
    }
}

fn format_gb(value: Option<f64>) -> String {
    match value {
        Some(value) if value >= 100.0 => format!("{value:.0}GB"),
        Some(value) if value >= 10.0 => format!("{value:.1}GB"),
        Some(value) => format!("{value:.2}GB"),
        None => "unknown".to_owned(),
    }
}

fn system_profile_summary(config: &AppConfig) -> String {
    let storage_path = get_ollama_models_path(config).unwrap_or_else(|| PathBuf::from("~/models"));
    let profile = build_system_profile(storage_path);
    let ram = profile
        .total_ram_gb
        .map(|value| format!("{} RAM", format_gb(Some(value))))
        .unwrap_or_else(|| "RAM unknown".to_owned());
    let storage = profile
        .storage_free_gb
        .map(|value| format!("{} free", format_gb(Some(value))))
        .unwrap_or_else(|| "storage unknown".to_owned());
    format!(
        "{} · {} · {} CPU · {} · {}",
        profile.system, profile.machine, profile.cpu_count, ram, storage
    )
}

fn model_picker_description(
    _model_name: &str,
    info: &vibn_core::ModelInfo,
    profile: &vibn_core::SystemProfile,
) -> String {
    let use_cases = info
        .use_cases
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let ram = format!("{}GB+ RAM", info.recommended_ram_gb.max(info.min_ram_gb));
    format!(
        "{} · {} · {} · {}",
        info.summary,
        use_cases,
        ram,
        model_fit(info, profile).as_str()
    )
}

fn configure_ollama_command(command: &mut Command, config: &AppConfig) {
    if !config.ollama_models_path.trim().is_empty() {
        command.env("OLLAMA_MODELS", &config.ollama_models_path);
    }
}

fn list_ollama_models(config: &AppConfig, subcommand: &str) -> BTreeSet<String> {
    let mut command = Command::new("ollama");
    command.arg(subcommand);
    configure_ollama_command(&mut command, config);
    let Ok(output) = command.output() else {
        return BTreeSet::new();
    };
    if !output.status.success() {
        return BTreeSet::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
        .collect()
}

fn list_installed_models(config: &AppConfig) -> BTreeSet<String> {
    list_ollama_models(config, "list")
}

fn list_loaded_models(config: &AppConfig) -> BTreeSet<String> {
    list_ollama_models(config, "ps")
}

fn discover_model_storage_devices() -> Vec<PathBuf> {
    let roots = if cfg!(target_os = "macos") {
        vec![PathBuf::from("/Volumes")]
    } else if cfg!(target_os = "linux") {
        let user = env::var("USER").unwrap_or_default();
        if user.is_empty() {
            vec![PathBuf::from("/mnt"), PathBuf::from("/media")]
        } else {
            vec![
                PathBuf::from(format!("/run/media/{user}")),
                PathBuf::from(format!("/media/{user}")),
                PathBuf::from("/mnt"),
            ]
        }
    } else {
        Vec::new()
    };

    let mut devices = roots
        .into_iter()
        .filter_map(|root| fs::read_dir(root).ok())
        .flat_map(|entries| entries.filter_map(|entry| entry.ok().map(|item| item.path())))
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    devices.sort();
    devices
}

fn ensure_model_available(config: &AppConfig, model_name: &str) -> Result<(), String> {
    if list_installed_models(config).contains(model_name) {
        return Ok(());
    }

    let registry = load_model_registry().map_err(|error| error.to_string())?;
    let Some(info) = registry.get(model_name) else {
        return Err(format!("Unknown model: {model_name}"));
    };

    if let Some(gguf) = &info.gguf {
        let models_dir =
            get_ollama_models_path(config).unwrap_or_else(|| PathBuf::from("~/models"));
        fs::create_dir_all(&models_dir).map_err(|error| error.to_string())?;
        let dest = models_dir.join(&gguf.file);
        if !dest.exists() {
            let status = Command::new("curl")
                .args(["-L", &gguf.url, "-o"])
                .arg(&dest)
                .status()
                .map_err(|error| format!("failed to download {model_name}: {error}"))?;
            if !status.success() {
                return Err(format!("failed to download {model_name} GGUF"));
            }
        }

        let modelfile_path = models_dir.join(format!("Modelfile-{model_name}"));
        fs::write(
            &modelfile_path,
            format!(
                "FROM {}\nSYSTEM \"\"\"{}\"\"\"\n",
                dest.display(),
                gguf.prompt
            ),
        )
        .map_err(|error| error.to_string())?;

        let mut command = Command::new("ollama");
        command.args(["create", model_name, "-f"]);
        command.arg(&modelfile_path);
        configure_ollama_command(&mut command, config);
        let status = command
            .status()
            .map_err(|error| format!("failed to create {model_name}: {error}"))?;
        if !status.success() {
            return Err(format!("ollama create failed for {model_name}"));
        }
        return Ok(());
    }

    let mut command = Command::new("ollama");
    command.args(["pull", model_name]);
    configure_ollama_command(&mut command, config);
    let status = command
        .status()
        .map_err(|error| format!("failed to pull {model_name}: {error}"))?;
    if !status.success() {
        return Err(format!("ollama pull failed for {model_name}"));
    }
    Ok(())
}

fn delete_installed_model(config: &AppConfig, model_name: &str) -> Result<(), String> {
    let mut command = Command::new("ollama");
    command.args(["rm", model_name]);
    configure_ollama_command(&mut command, config);
    let output = command
        .output()
        .map_err(|error| format!("failed to delete {model_name}: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if detail.is_empty() {
            format!("could not delete {model_name}")
        } else {
            detail
        })
    }
}

fn switch_model(
    state: &mut UiState,
    model_name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    ensure_model_available(&state.config, model_name).map_err(io::Error::other)?;
    state.model = model_name.to_owned();
    state.config.default_model = state.model.clone();
    save_session_messages(state, &[])?;
    state.reset_output();
    Ok(format!("Switched to {}. Conversation reset.", state.model))
}

fn browser_view_title(view: &BrowserView) -> String {
    match view {
        BrowserView::Skills => "Skills".to_owned(),
        BrowserView::ModelPicker => "Models".to_owned(),
        BrowserView::StoragePicker => "Model Storage Path".to_owned(),
        BrowserView::StorageDevices => "Mounted Devices".to_owned(),
        BrowserView::McpManager => "MCP Servers".to_owned(),
        BrowserView::MarketTop => "MCP Marketplace".to_owned(),
        BrowserView::MarketCategory(category) => {
            format!("Marketplace > {}", category_label(category))
        }
        BrowserView::MarketAll => "All MCP Servers".to_owned(),
        BrowserView::MarketSearch(query) => format!("Search: {query}"),
    }
}

fn browser_view_footer(view: &BrowserView) -> String {
    match view {
        BrowserView::Skills => "  ↑↓/jk navigate  Enter activate  / filter  q back ".to_owned(),
        BrowserView::ModelPicker => {
            "  ↑↓/jk navigate  Enter switch/install  d delete installed  / filter  q back "
                .to_owned()
        }
        BrowserView::StoragePicker | BrowserView::StorageDevices => {
            "  ↑↓/jk navigate  Enter select path  / filter  q back ".to_owned()
        }
        BrowserView::McpManager => {
            "  ↑↓/jk navigate  Enter toggle/action  / filter  q back ".to_owned()
        }
        BrowserView::MarketTop => "  ↑↓/jk navigate  Enter select  / filter  q back ".to_owned(),
        _ => "  ↑↓/jk navigate  Enter install/connect  / filter  q back ".to_owned(),
    }
}

fn browser_items_for_view(view: &BrowserView, config: &AppConfig) -> (String, Vec<BrowserItem>) {
    let connected = list_connected_mcp_servers().unwrap_or_default();
    let configured = configured_mcp_servers(config);
    let connected_names = connected
        .iter()
        .map(|status| status.name.clone())
        .collect::<Vec<_>>();
    let mut items = Vec::new();

    match view {
        BrowserView::Skills => {
            for skill in SKILLS {
                items.push(BrowserItem {
                    label: skill.name.to_owned(),
                    description: skill.description.to_owned(),
                    meta: skill_category_label(skill.category).to_owned(),
                    style: BrowserItemStyle::Normal,
                    selectable: true,
                    action: BrowserAction::SkillActivate(skill.key.to_owned()),
                });
            }
            ("Select a skill to activate".to_owned(), items)
        }
        BrowserView::ModelPicker => {
            let installed = list_installed_models(config);
            let loaded = list_loaded_models(config);
            let storage_path =
                get_ollama_models_path(config).unwrap_or_else(|| PathBuf::from("~/models"));
            let profile = build_system_profile(storage_path.clone());
            items.push(BrowserItem {
                label: "Model storage path".to_owned(),
                description: storage_path.display().to_string(),
                meta: format!("{} free · config", format_gb(profile.storage_free_gb)),
                style: BrowserItemStyle::Installed,
                selectable: true,
                action: BrowserAction::ModelStorage,
            });
            if let Ok(registry) = load_model_registry() {
                for (model_name, info) in registry {
                    let mut meta = Vec::new();
                    if model_name == config.default_model {
                        meta.push("active".to_owned());
                    }
                    if loaded.contains(&model_name) {
                        meta.push("loaded".to_owned());
                    }
                    if installed.contains(&model_name) {
                        meta.push("installed".to_owned());
                    }
                    meta.push(format_gb(Some(info.size_gb)));
                    meta.push(if info.tool_support {
                        "tools".to_owned()
                    } else {
                        "no tools".to_owned()
                    });
                    meta.push(model_fit(&info, &profile).as_str().to_owned());
                    items.push(BrowserItem {
                        label: model_name.clone(),
                        description: model_picker_description(&model_name, &info, &profile),
                        meta: meta.join(" · "),
                        style: if model_name == config.default_model {
                            BrowserItemStyle::Connected
                        } else if installed.contains(&model_name) {
                            BrowserItemStyle::Installed
                        } else {
                            BrowserItemStyle::Normal
                        },
                        selectable: true,
                        action: BrowserAction::ModelSwitch(model_name),
                    });
                }
            }
            (
                format!(
                    "Current: {} · {}",
                    config.default_model,
                    system_profile_summary(config)
                ),
                items,
            )
        }
        BrowserView::StoragePicker => {
            let mut seen = BTreeSet::new();
            let mut add_item = |path: PathBuf,
                                label: &str,
                                description: String,
                                meta: &str,
                                items: &mut Vec<BrowserItem>| {
                if !seen.insert(path.clone()) {
                    return;
                }
                let profile = build_system_profile(path.clone());
                let mut meta_parts = Vec::new();
                if !meta.is_empty() {
                    meta_parts.push(meta.to_owned());
                }
                if profile.storage_free_gb.is_some() {
                    meta_parts.push(format!("{} free", format_gb(profile.storage_free_gb)));
                }
                items.push(BrowserItem {
                    label: label.to_owned(),
                    description,
                    meta: meta_parts.join(" · "),
                    style: BrowserItemStyle::Normal,
                    selectable: true,
                    action: BrowserAction::StorageSetPath(path.display().to_string()),
                });
            };
            if let Some(configured_path) = get_ollama_models_path(config) {
                add_item(
                    configured_path.clone(),
                    "Current configured path",
                    configured_path.display().to_string(),
                    "current",
                    &mut items,
                );
            }
            let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            add_item(
                cwd.clone(),
                "Current working directory",
                cwd.display().to_string(),
                "default",
                &mut items,
            );
            add_item(
                PathBuf::from("~/models"),
                "Built-in local models folder",
                "~/models".to_owned(),
                "local",
                &mut items,
            );
            let devices = discover_model_storage_devices();
            if !devices.is_empty() {
                items.push(BrowserItem {
                    label: "Browse mounted devices".to_owned(),
                    description: "Thumb drives, SD cards, and other mounted volumes".to_owned(),
                    meta: format!(
                        "{} device{}",
                        devices.len(),
                        if devices.len() == 1 { "" } else { "s" }
                    ),
                    style: BrowserItemStyle::Installed,
                    selectable: true,
                    action: BrowserAction::StorageOpenDevices,
                });
            }
            (
                "Current working directory is the default quick-pick".to_owned(),
                items,
            )
        }
        BrowserView::StorageDevices => {
            for path in discover_model_storage_devices() {
                let profile = build_system_profile(path.clone());
                items.push(BrowserItem {
                    label: path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("device")
                        .to_owned(),
                    description: path.display().to_string(),
                    meta: format!("device · {} free", format_gb(profile.storage_free_gb)),
                    style: BrowserItemStyle::Normal,
                    selectable: true,
                    action: BrowserAction::StorageSetPath(path.display().to_string()),
                });
            }
            ("Select a device root for model storage".to_owned(), items)
        }
        BrowserView::McpManager => {
            for status in &connected {
                let description = if status.args.is_empty() {
                    status.command.clone()
                } else {
                    format!("{} {}", status.command, status.args.join(" "))
                };
                items.push(BrowserItem {
                    label: status.name.clone(),
                    description,
                    meta: format!("connected ({} tools)", status.tool_count),
                    style: BrowserItemStyle::Connected,
                    selectable: true,
                    action: BrowserAction::McpToggle(status.name.clone()),
                });
            }

            let mut configured_names = configured.keys().cloned().collect::<Vec<_>>();
            configured_names.sort();
            for name in configured_names {
                if connected_names
                    .iter()
                    .any(|connected_name| connected_name == &name)
                {
                    continue;
                }
                let server = configured
                    .get(&name)
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                let command = server
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or("<missing command>");
                let args = server
                    .get("args")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(Value::as_str)
                            .take(3)
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .unwrap_or_default();
                let description = if args.is_empty() {
                    command.to_owned()
                } else {
                    format!("{command} {args}")
                };
                items.push(BrowserItem {
                    label: name.clone(),
                    description,
                    meta: "installed".to_owned(),
                    style: BrowserItemStyle::Installed,
                    selectable: true,
                    action: BrowserAction::McpToggle(name),
                });
            }

            if items.is_empty() {
                items.push(BrowserItem {
                    label: "No MCP servers configured".to_owned(),
                    description: "Use /market to browse and install servers".to_owned(),
                    meta: String::new(),
                    style: BrowserItemStyle::Normal,
                    selectable: true,
                    action: BrowserAction::McpMarket,
                });
            }

            items.push(BrowserItem {
                label: "+ Browse Marketplace".to_owned(),
                description: "Find and install new MCP servers".to_owned(),
                meta: String::new(),
                style: BrowserItemStyle::Installed,
                selectable: true,
                action: BrowserAction::McpMarket,
            });
            items.push(BrowserItem {
                label: "+ Connect All".to_owned(),
                description: "Connect all installed servers".to_owned(),
                meta: String::new(),
                style: BrowserItemStyle::Installed,
                selectable: true,
                action: BrowserAction::McpConnectAll,
            });
            items.push(BrowserItem {
                label: "- Disconnect All".to_owned(),
                description: "Disconnect all active servers".to_owned(),
                meta: String::new(),
                style: BrowserItemStyle::Normal,
                selectable: true,
                action: BrowserAction::McpDisconnectAll,
            });

            (
                format!(
                    "{} connected, {} configured",
                    connected.len(),
                    configured.len()
                ),
                items,
            )
        }
        BrowserView::MarketTop => {
            if !connected.is_empty() || !configured.is_empty() {
                let mut active_items = Vec::new();
                for status in &connected {
                    let suffix = if status.tool_count == 1 { "" } else { "s" };
                    active_items.push(BrowserItem {
                        label: status.name.clone(),
                        description: format!("{} tool{} available", status.tool_count, suffix),
                        meta: "connected".to_owned(),
                        style: BrowserItemStyle::Connected,
                        selectable: true,
                        action: BrowserAction::MarketToggle(status.name.clone()),
                    });
                }
                let mut configured_only = configured.keys().cloned().collect::<Vec<_>>();
                configured_only.sort();
                for name in configured_only {
                    if connected_names
                        .iter()
                        .any(|connected_name| connected_name == &name)
                    {
                        continue;
                    }
                    active_items.push(BrowserItem {
                        label: name.clone(),
                        description: "Saved in config, not connected".to_owned(),
                        meta: "installed".to_owned(),
                        style: BrowserItemStyle::Installed,
                        selectable: true,
                        action: BrowserAction::MarketToggle(name),
                    });
                }
                if !active_items.is_empty() {
                    items.extend(active_items);
                    items.push(BrowserItem {
                        label: "── Categories ──".to_owned(),
                        description: String::new(),
                        meta: String::new(),
                        style: BrowserItemStyle::Separator,
                        selectable: false,
                        action: BrowserAction::None,
                    });
                }
            }

            for (category_id, category_name) in MARKET_CATEGORIES {
                let entries = market_entries_by_category(category_id);
                let count = entries.len();
                let connected_count = entries
                    .iter()
                    .filter(|entry| connected_names.iter().any(|name| name == entry.name))
                    .count();
                let installed_count = entries
                    .iter()
                    .filter(|entry| configured.contains_key(entry.name))
                    .count();
                let mut meta = format!("{count} servers");
                if connected_count > 0 {
                    meta.push_str(&format!(" ({connected_count} active)"));
                } else if installed_count > 0 {
                    meta.push_str(&format!(" ({installed_count} ready)"));
                }
                items.push(BrowserItem {
                    label: (*category_name).to_owned(),
                    description: format!("Browse {} MCP servers", category_name.to_lowercase()),
                    meta,
                    style: BrowserItemStyle::Normal,
                    selectable: true,
                    action: BrowserAction::MarketOpenCategory((*category_id).to_owned()),
                });
            }

            let connected_count = connected.len();
            let installed_count = configured.len().saturating_sub(connected_count);
            let subtitle = if connected_count == 0 && installed_count == 0 {
                "Browse and install MCP servers".to_owned()
            } else {
                let mut parts = Vec::new();
                if connected_count > 0 {
                    parts.push(format!("{connected_count} connected"));
                }
                if installed_count > 0 {
                    parts.push(format!("{installed_count} installed"));
                }
                format!("Browse and install MCP servers ({})", parts.join(", "))
            };
            (subtitle, items)
        }
        BrowserView::MarketCategory(category) => {
            let entries = market_entries_by_category(category);
            for entry in &entries {
                let connected_here = connected_names.iter().any(|name| name == entry.name);
                let installed_here = configured.contains_key(entry.name);
                let auth = if entry.env_vars.is_empty() {
                    "no auth".to_owned()
                } else {
                    "needs key".to_owned()
                };
                let meta = if connected_here {
                    format!("connected | {auth}")
                } else if installed_here {
                    format!("installed | {auth}")
                } else {
                    auth
                };
                items.push(BrowserItem {
                    label: entry.name.to_owned(),
                    description: entry.description.to_owned(),
                    meta,
                    style: if connected_here {
                        BrowserItemStyle::Connected
                    } else if installed_here {
                        BrowserItemStyle::Installed
                    } else {
                        BrowserItemStyle::Normal
                    },
                    selectable: true,
                    action: BrowserAction::MarketInstall(entry.name.to_owned()),
                });
            }
            (format!("{} servers available", items.len()), items)
        }
        BrowserView::MarketAll => {
            for entry in MARKETPLACE {
                let connected_here = connected_names.iter().any(|name| name == entry.name);
                let installed_here = configured.contains_key(entry.name);
                let mut meta = category_label(entry.category).to_owned();
                if connected_here {
                    meta = format!("connected | {meta}");
                } else if installed_here {
                    meta = format!("installed | {meta}");
                }
                items.push(BrowserItem {
                    label: entry.name.to_owned(),
                    description: entry.description.to_owned(),
                    meta,
                    style: if connected_here {
                        BrowserItemStyle::Connected
                    } else if installed_here {
                        BrowserItemStyle::Installed
                    } else {
                        BrowserItemStyle::Normal
                    },
                    selectable: true,
                    action: BrowserAction::MarketInstall(entry.name.to_owned()),
                });
            }
            (format!("{} servers", items.len()), items)
        }
        BrowserView::MarketSearch(query) => {
            for entry in search_marketplace(query) {
                let connected_here = connected_names.iter().any(|name| name == entry.name);
                let installed_here = configured.contains_key(entry.name);
                let mut meta = category_label(entry.category).to_owned();
                if connected_here {
                    meta = format!("connected | {meta}");
                } else if installed_here {
                    meta = format!("installed | {meta}");
                }
                items.push(BrowserItem {
                    label: entry.name.to_owned(),
                    description: entry.description.to_owned(),
                    meta,
                    style: if connected_here {
                        BrowserItemStyle::Connected
                    } else if installed_here {
                        BrowserItemStyle::Installed
                    } else {
                        BrowserItemStyle::Normal
                    },
                    selectable: true,
                    action: BrowserAction::MarketInstall(entry.name.to_owned()),
                });
            }
            (format!("{} results", items.len()), items)
        }
    }
}

fn filter_browser_items(items: &[BrowserItem], filter: &str) -> Vec<BrowserItem> {
    if filter.trim().is_empty() {
        return items.to_vec();
    }
    let filter = filter.to_lowercase();
    items
        .iter()
        .filter(|item| {
            item.selectable
                && (item.label.to_lowercase().contains(&filter)
                    || item.description.to_lowercase().contains(&filter)
                    || item.meta.to_lowercase().contains(&filter))
        })
        .cloned()
        .collect()
}

fn refresh_browser_mode(state: &mut UiState) {
    if let Some(browser) = state.browser_mode.as_mut() {
        let (subtitle, items) = browser_items_for_view(&browser.view, &state.config);
        browser.title = browser_view_title(&browser.view);
        browser.subtitle = subtitle;
        browser.footer = browser_view_footer(&browser.view);
        browser.items = filter_browser_items(&items, &browser.filter);
        if browser.items.is_empty() {
            browser.selected = 0;
        } else {
            browser.selected = browser.selected.min(browser.items.len() - 1);
            if !browser.items[browser.selected].selectable {
                browser.selected = next_selectable_index(&browser.items, browser.selected, 1)
                    .unwrap_or(browser.selected);
            }
        }
    }
}

fn open_browser_mode(
    state: &mut UiState,
    input: &mut TextArea<'static>,
    view: BrowserView,
    back_view: Option<BrowserView>,
) {
    state.browser_mode = Some(BrowserMode {
        view,
        back_view,
        title: String::new(),
        subtitle: String::new(),
        footer: String::new(),
        items: Vec::new(),
        selected: 0,
        filter: String::new(),
        filtering: false,
    });
    state.auto_scroll = false;
    state.scroll_offset = 0;
    *input = build_input();
    refresh_browser_mode(state);
}

fn close_browser_mode(state: &mut UiState, input: &mut TextArea<'static>) {
    state.browser_mode = None;
    *input = build_input();
}

fn next_selectable_index(items: &[BrowserItem], start: usize, direction: isize) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    let mut index = start as isize;
    for _ in 0..items.len() {
        index = (index + direction).rem_euclid(items.len() as isize);
        if items[index as usize].selectable {
            return Some(index as usize);
        }
    }
    None
}

fn render_browser_lines(browser: &BrowserMode, width: u16) -> Vec<String> {
    if matches!(browser.view, BrowserView::ModelPicker) {
        return render_model_browser_lines(browser, width);
    }

    let mut lines = vec![
        String::new(),
        format!("  {}", browser.title),
        format!("  {}", browser.subtitle),
        if browser.filter.trim().is_empty() {
            "  Filter: (none)".to_owned()
        } else {
            format!("  Filter: {}", browser.filter)
        },
        "  ───────────────────────────────────────────────────────────".to_owned(),
    ];
    if browser.items.is_empty() {
        lines.push("  No items matched.".to_owned());
        return lines;
    }
    for (index, item) in browser.items.iter().enumerate() {
        if !item.selectable {
            lines.push(format!("  {}", item.label));
            continue;
        }
        let marker = if index == browser.selected {
            "▶"
        } else {
            " "
        };
        let label = match item.style {
            BrowserItemStyle::Connected => format!("{} ●", item.label),
            BrowserItemStyle::Installed => format!("{} ○", item.label),
            _ => item.label.clone(),
        };
        if item.meta.is_empty() {
            lines.push(format!("  {marker} {label}"));
        } else {
            lines.push(format!("  {marker} {:<24} {}", label, item.meta));
        }
        if !item.description.is_empty() {
            lines.push(format!("    {}", item.description));
        }
        lines.push(String::new());
    }
    lines
}

fn render_model_browser_lines(browser: &BrowserMode, width: u16) -> Vec<String> {
    let width = usize::from(width).max(40);
    let table_width = width.saturating_sub(4).clamp(40, 112).min(width);
    let (model_width, state_width, size_width, tools_width, fit_width) = if table_width >= 108 {
        (30, 16, 8, 4, 10)
    } else if table_width >= 78 {
        (20, 9, 7, 4, 7)
    } else {
        (16, 8, 6, 4, 6)
    };
    let fixed_width =
        21 + model_width + state_width + size_width + tools_width + fit_width;
    let best_width = table_width.saturating_sub(fixed_width).max(6);
    let widths = ModelTableWidths {
        model: model_width,
        state: state_width,
        size: size_width,
        tools: tools_width,
        fit: fit_width,
        best: best_width,
        total: table_width,
    };

    let mut lines = vec![
        String::new(),
        truncate_line(&format!("  {}", browser.title), width),
        truncate_line(&format!("  {}", browser.subtitle), width),
        if browser.filter.trim().is_empty() {
            "  Filter: (none)".to_owned()
        } else {
            truncate_line(&format!("  Filter: {}", browser.filter), width)
        },
        center_line(
            format_model_table_border("┌", "┬", "┐", widths),
            width,
        ),
        center_line(
            format_model_table_row(
                " ",
                "Model",
                "State",
                "Size",
                "Tool",
                "Fit",
                "Best for",
                widths,
            ),
            width,
        ),
        center_line(
            format_model_table_border("├", "┼", "┤", widths),
            width,
        ),
    ];

    if browser.items.is_empty() {
        lines.push("  No items matched.".to_owned());
        lines.push(center_line(
            format_model_table_border("└", "┴", "┘", widths),
            width,
        ));
        return lines;
    }

    for (index, item) in browser.items.iter().enumerate() {
        if !item.selectable {
            lines.push(truncate_line(&format!("  {}", item.label), width));
            continue;
        }
        let marker = if index == browser.selected { "▶" } else { " " };
        let (model, state, size, tools, fit, best_for) = model_table_cells(item);
        lines.push(center_line(
            format_model_table_row(
                marker,
                &model,
                &state,
                &size,
                &tools,
                &fit,
                &best_for,
                widths,
            ),
            width,
        ));
    }
    lines.push(center_line(
        format_model_table_border("└", "┴", "┘", widths),
        width,
    ));

    lines
}

#[derive(Clone, Copy)]
struct ModelTableWidths {
    model: usize,
    state: usize,
    size: usize,
    tools: usize,
    fit: usize,
    best: usize,
    total: usize,
}

fn format_model_table_row(
    marker: &str,
    model: &str,
    state: &str,
    size: &str,
    tools: &str,
    fit: &str,
    best_for: &str,
    widths: ModelTableWidths,
) -> String {
    truncate_line(
        &format!(
            "  │ {} {:<model_width$} │ {:<state_width$} │ {:>size_width$} │ {:<tools_width$} │ {:<fit_width$} │ {:<best_width$} │",
            marker,
            fit_cell(model, widths.model),
            fit_cell(state, widths.state),
            fit_cell(size, widths.size),
            fit_cell(tools, widths.tools),
            fit_cell(fit, widths.fit),
            fit_cell(best_for, widths.best),
            model_width = widths.model,
            state_width = widths.state,
            size_width = widths.size,
            tools_width = widths.tools,
            fit_width = widths.fit,
            best_width = widths.best,
        ),
        widths.total,
    )
}

fn format_model_table_border(
    left: &str,
    junction: &str,
    right: &str,
    widths: ModelTableWidths,
) -> String {
    truncate_line(
        &format!(
            "  {left}{model}{junction}{state}{junction}{size}{junction}{tools}{junction}{fit}{junction}{best}{right}",
            model = "─".repeat(widths.model + 4),
            state = "─".repeat(widths.state + 2),
            size = "─".repeat(widths.size + 2),
            tools = "─".repeat(widths.tools + 2),
            fit = "─".repeat(widths.fit + 2),
            best = "─".repeat(widths.best + 2),
        ),
        widths.total,
    )
}

fn center_line(value: String, width: usize) -> String {
    let value_width = value.chars().count();
    if value_width >= width {
        return value;
    }
    format!("{}{}", " ".repeat((width - value_width) / 2), value)
}

fn model_table_cells(item: &BrowserItem) -> (String, String, String, String, String, String) {
    if matches!(item.action, BrowserAction::ModelStorage) {
        let mut meta = item.meta.split(" · ");
        let size = meta.next().unwrap_or("").to_owned();
        let state = meta.collect::<Vec<_>>().join(", ");
        return (
            item.label.clone(),
            if state.is_empty() {
                "config".to_owned()
            } else {
                state
            },
            size,
            String::new(),
            String::new(),
            item.description.clone(),
        );
    }

    let meta = item.meta.split(" · ").collect::<Vec<_>>();
    let states = meta
        .iter()
        .copied()
        .filter(|value| matches!(*value, "active" | "loaded" | "installed"))
        .collect::<Vec<_>>();
    let state = if states.is_empty() {
        "-".to_owned()
    } else {
        states.join(", ")
    };
    let size = meta
        .iter()
        .copied()
        .find(|value| value.ends_with("GB") || value == &"unknown")
        .unwrap_or("")
        .to_owned();
    let tools = if meta.iter().any(|value| *value == "no tools") {
        "no".to_owned()
    } else if meta.iter().any(|value| *value == "tools") {
        "yes".to_owned()
    } else {
        String::new()
    };
    let fit = meta.last().copied().unwrap_or("").to_owned();
    let best_for = item
        .description
        .split(" · ")
        .next()
        .unwrap_or("")
        .to_owned();

    (item.label.clone(), state, size, tools, fit, best_for)
}

fn fit_cell(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let char_count = value.chars().count();
    if char_count <= width {
        return value.to_owned();
    }
    if width == 1 {
        return "…".to_owned();
    }
    let mut result = value.chars().take(width - 1).collect::<String>();
    result.push('…');
    result
}

fn truncate_line(value: &str, width: usize) -> String {
    fit_cell(value, width)
}

fn current_install_prompt(mode: &InstallPromptMode) -> Option<String> {
    mode.steps.get(mode.step_index).map(|step| match step {
        InstallPromptStep::Placeholder { name, .. } => name.clone(),
        InstallPromptStep::EnvVar(name) => name.clone(),
    })
}

fn build_install_prompt_mode(entry: &MarketCatalogEntry) -> InstallPromptMode {
    let mut resolved_args = Vec::with_capacity(entry.args.len());
    let mut steps = Vec::new();
    for (index, arg) in entry.args.iter().enumerate() {
        if arg.starts_with('{') && arg.ends_with('}') {
            resolved_args.push(None);
            steps.push(InstallPromptStep::Placeholder {
                name: arg[1..arg.len() - 1].to_owned(),
                arg_index: index,
            });
        } else {
            resolved_args.push(Some((*arg).to_owned()));
        }
    }

    for env_var in entry.env_vars {
        if env::var(env_var)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .is_none()
        {
            steps.push(InstallPromptStep::EnvVar((*env_var).to_owned()));
        }
    }

    InstallPromptMode {
        server_name: entry.name.to_owned(),
        command: entry.command.to_owned(),
        description: entry.description.to_owned(),
        resolved_args,
        required_env_vars: entry
            .env_vars
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        env: Map::new(),
        steps,
        step_index: 0,
    }
}

fn save_marketplace_server_config(
    state: &mut UiState,
    install: &InstallPromptMode,
    resolved_args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut server = Map::new();
    server.insert("command".to_owned(), Value::String(install.command.clone()));
    server.insert(
        "args".to_owned(),
        Value::Array(
            resolved_args
                .iter()
                .map(|value| Value::String(value.clone()))
                .collect(),
        ),
    );
    if !install.required_env_vars.is_empty() {
        server.insert(
            "env_vars".to_owned(),
            Value::Array(
                install
                    .required_env_vars
                    .iter()
                    .map(|value| Value::String(value.clone()))
                    .collect(),
            ),
        );
    }
    if !install.env.is_empty() {
        server.insert("env".to_owned(), Value::Object(install.env.clone()));
    }

    let mut servers = configured_mcp_servers(&state.config);
    servers.insert(install.server_name.clone(), Value::Object(server));
    state
        .config
        .extra
        .insert("mcp_servers".to_owned(), Value::Object(servers));
    save_config(&state.config)?;
    Ok(())
}

fn finish_install_prompt(state: &mut UiState) -> Result<String, Box<dyn std::error::Error>> {
    let Some(install) = state.install_prompt.take() else {
        return Ok(String::new());
    };
    let resolved_args = install
        .resolved_args
        .iter()
        .cloned()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| "missing marketplace args".to_owned())?;
    save_marketplace_server_config(state, &install, &resolved_args)?;
    let tool_count = connect_mcp_server(
        &install.server_name,
        &install.command,
        &resolved_args,
        &install.env,
    )
    .map_err(|error| {
        format!(
            "Saved {} but failed to connect: {error}",
            install.server_name
        )
    })?;
    refresh_browser_mode(state);
    Ok(format!(
        "Installed {} — {} ({} tool{})",
        install.server_name,
        install.description,
        tool_count,
        if tool_count == 1 { "" } else { "s" }
    ))
}

fn start_marketplace_install(
    state: &mut UiState,
    input: &mut TextArea<'static>,
    name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let entry = resolve_marketplace_matches(name).map_err(|matches| {
        if matches.is_empty() {
            format!("Not found: {name}")
        } else {
            let mut lines = vec!["Multiple matches:".to_owned()];
            lines.extend(
                matches
                    .iter()
                    .map(|entry| format!("  {} — {}", entry.name, entry.description)),
            );
            lines.join("\n")
        }
    })?;
    let install = build_install_prompt_mode(entry);
    if install.steps.is_empty() {
        state.install_prompt = Some(install);
        return finish_install_prompt(state);
    }
    state.install_prompt = Some(install);
    *input = build_input();
    Ok(format!(
        "Installing {} — enter requested values",
        entry.name
    ))
}

fn activate_skill_into_session(
    state: &mut UiState,
    key: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let skill = resolve_skill_matches(key).map_err(|matches| {
        if matches.is_empty() {
            format!("Unknown skill: {key}")
        } else {
            let mut lines = vec![format!("Unknown skill: {key}"), "Did you mean:".to_owned()];
            lines.extend(matches.iter().map(|value| format!("  {value}")));
            lines.join("\n")
        }
    })?;

    let (_, mut messages) = load_transcript(&state.session_id)?;
    messages.extend(skill_activation_messages(skill));
    save_session_messages(state, &messages)?;
    state.output_lines = render_messages(&messages);
    state.scroll_offset = 0;
    state.auto_scroll = true;
    Ok(format!("Activated: {} — {}", skill.name, skill.description))
}

fn skill_activation_messages(skill: &SkillCatalogEntry) -> [ChatMessage; 2] {
    [
        ChatMessage::user(format!(
            "[SKILL ACTIVATED: {}]\n{}",
            skill.name, skill.prompt
        )),
        ChatMessage::assistant(format!(
            "Understood. I'm now in **{}** mode. What would you like me to work on?",
            skill.name
        )),
    ]
}

fn load_constraints() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let path = constraints_file_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    Ok(value
        .get("hard_rules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect())
}

fn current_review_example_mut(state: &mut UiState) -> Option<&mut Value> {
    state.review_mode.examples.get_mut(state.review_mode.index)
}

fn exit_review_mode(state: &mut UiState, done: bool) {
    let approved = state.review_mode.approved;
    let discarded = state.review_mode.discarded;
    let skipped = state.review_mode.skipped;
    let approved_file = state.review_mode.approved_file.display().to_string();
    state.review_mode.active = false;
    state.review_mode.examples.clear();
    state.review_mode.index = 0;
    state.review_mode.staging_file.clear();
    state.auto_scroll = true;
    state.scroll_offset = 0;
    if done {
        state.append(format!(
            "Review complete!  Approved: {approved}  Discarded: {discarded}  Skipped: {skipped}"
        ));
        state.append(format!("Saved to: {approved_file}"));
    } else {
        state.append(format!(
            "Review paused.  Approved: {approved}  Discarded: {discarded}  Skipped: {skipped}"
        ));
    }
    state.append(String::new());
}

fn review_action(state: &mut UiState, action: &str) -> Result<(), Box<dyn std::error::Error>> {
    if state.review_mode.examples.is_empty() {
        return Ok(());
    }
    let current_index = state.review_mode.index;
    match action {
        "approve" => {
            let clean = clean_review_example(&state.review_mode.examples[current_index]);
            append_jsonl_line(&state.review_mode.approved_file, &clean)?;
            state.review_mode.approved += 1;
            if let Some(meta) = current_review_example_mut(state)
                .and_then(|value| value.as_object_mut())
                .map(|object| {
                    object
                        .entry("_meta".to_owned())
                        .or_insert_with(|| Value::Object(Map::new()))
                })
                .and_then(Value::as_object_mut)
            {
                meta.insert("reviewed".to_owned(), Value::Bool(true));
                meta.insert("verdict".to_owned(), Value::String("approved".to_owned()));
            }
        }
        "discard" => {
            state.review_mode.discarded += 1;
            if let Some(meta) = current_review_example_mut(state)
                .and_then(|value| value.as_object_mut())
                .map(|object| {
                    object
                        .entry("_meta".to_owned())
                        .or_insert_with(|| Value::Object(Map::new()))
                })
                .and_then(Value::as_object_mut)
            {
                meta.insert("reviewed".to_owned(), Value::Bool(true));
                meta.insert("verdict".to_owned(), Value::String("discarded".to_owned()));
            }
        }
        "skip" => {
            state.review_mode.skipped += 1;
        }
        "quit" => {
            exit_review_mode(state, false);
            return Ok(());
        }
        _ => return Ok(()),
    }

    let mut next_index = current_index + 1;
    while next_index < state.review_mode.examples.len() {
        if !example_reviewed(&state.review_mode.examples[next_index]) {
            break;
        }
        next_index += 1;
    }
    if next_index >= state.review_mode.examples.len() {
        exit_review_mode(state, true);
    } else {
        state.review_mode.index = next_index;
        state.auto_scroll = false;
        state.scroll_offset = 0;
    }
    Ok(())
}

fn edit_current_review_example(state: &mut UiState) -> Result<(), Box<dyn std::error::Error>> {
    let Some(example) = current_review_example_mut(state) else {
        return Ok(());
    };
    let Some(conversations) = example
        .get_mut("conversations")
        .and_then(Value::as_array_mut)
    else {
        return Ok(());
    };
    let Some(gpt_turn) = conversations
        .iter_mut()
        .find(|turn| turn.get("from").and_then(Value::as_str) == Some("gpt"))
    else {
        return Ok(());
    };
    let Some(value) = gpt_turn.get("value").and_then(Value::as_str) else {
        return Ok(());
    };
    let path = temp_review_edit_path();
    fs::write(&path, value)?;
    let edit_result = open_in_editor(&path);
    if edit_result.is_ok() {
        let updated = fs::read_to_string(&path)?;
        if let Some(object) = gpt_turn.as_object_mut() {
            object.insert("value".to_owned(), Value::String(updated));
        }
    }
    let _ = fs::remove_file(&path);
    edit_result
}

fn save_constraints(rules: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path = constraints_file_path();
    let mut value = if path.exists() {
        serde_json::from_str::<Value>(&fs::read_to_string(&path)?)?
    } else {
        Value::Object(Map::new())
    };
    value["hard_rules"] = Value::Array(rules.iter().cloned().map(Value::String).collect());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(&value)?)?;
    Ok(())
}

fn render_constraints_lines(mode: &ConstraintsMode) -> Vec<String> {
    let mut lines = vec![
        String::new(),
        format!("  Constraints  {} rules", mode.rules.len()),
        "  ───────────────────────────────────────".to_owned(),
    ];
    if mode.rules.is_empty() {
        lines.push("  No constraints yet.".to_owned());
    } else {
        lines.extend(mode.rules.iter().enumerate().map(|(index, rule)| {
            if index == mode.selected {
                format!("  ▶ {rule}")
            } else {
                format!("    {rule}")
            }
        }));
    }
    lines.push(String::new());
    lines.push("  ↑↓ navigate   d delete   a add   e edit   q quit".to_owned());
    lines
}

fn latest_staging_file() -> Option<PathBuf> {
    let mut files = fs::read_dir(staging_dir_path())
        .ok()?
        .filter_map(|entry| entry.ok().map(|value| value.path()))
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .collect::<Vec<_>>();
    files.sort();
    files.pop()
}

fn load_review_examples(path: &Path) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    Ok(fs::read_to_string(path)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?)
}

fn example_reviewed(example: &Value) -> bool {
    example
        .get("_meta")
        .and_then(|value| value.get("reviewed"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn first_unreviewed_index(examples: &[Value]) -> usize {
    examples
        .iter()
        .position(|example| !example_reviewed(example))
        .unwrap_or(0)
}

fn render_review_lines(mode: &ReviewMode) -> Vec<String> {
    let mut lines = Vec::new();
    if mode.examples.is_empty() {
        return lines;
    }
    let example = &mode.examples[mode.index];
    let meta = example.get("_meta").and_then(Value::as_object);
    let source = meta
        .and_then(|value| value.get("source"))
        .and_then(Value::as_str)
        .unwrap_or("?");
    let score = meta
        .and_then(|value| value.get("score"))
        .map(|value| value.to_string())
        .unwrap_or_else(|| "?".to_owned());
    let max_score = meta
        .and_then(|value| value.get("max_score"))
        .map(|value| value.to_string())
        .unwrap_or_else(|| "?".to_owned());
    lines.push(String::new());
    lines.push(format!(
        "  Review {}/{}  source={}  score={}/{}  approved={} discarded={} skipped={}",
        mode.index + 1,
        mode.examples.len(),
        source,
        score,
        max_score,
        mode.approved,
        mode.discarded,
        mode.skipped
    ));
    lines.push("  ───────────────────────────────────────────────────────────".to_owned());
    lines.push("  y approve   n discard   s skip   e edit   q quit review".to_owned());
    lines.push(String::new());
    if let Some(conversations) = example.get("conversations").and_then(Value::as_array) {
        for turn in conversations {
            let role = turn.get("from").and_then(Value::as_str).unwrap_or("");
            let value = turn.get("value").and_then(Value::as_str).unwrap_or("");
            match role {
                "system" => lines.push(format!("  SYSTEM: {}", truncate_for_summary(value, 120))),
                "human" => {
                    lines.push("  USER:".to_owned());
                    lines.extend(value.lines().map(|line| format!("  {line}")));
                }
                "gpt" => {
                    lines.push(String::new());
                    lines.push("  ASSISTANT:".to_owned());
                    lines.extend(value.lines().map(|line| format!("  {line}")));
                }
                _ => {}
            }
            lines.push(String::new());
        }
    }
    lines
}

fn append_jsonl_line(path: &Path, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut payload = serde_json::to_string(value)?;
    payload.push('\n');
    if path.exists() {
        let mut content = fs::read(path)?;
        if !content.is_empty() && !content.ends_with(b"\n") {
            content.push(b'\n');
        }
        content.extend_from_slice(payload.as_bytes());
        fs::write(path, content)?;
    } else {
        fs::write(path, payload)?;
    }
    Ok(())
}

fn clean_review_example(example: &Value) -> Value {
    if let Some(object) = example.as_object() {
        let mut clean = object.clone();
        clean.remove("_meta");
        Value::Object(clean)
    } else {
        example.clone()
    }
}

fn parse_transcript_export_args(
    arg: &str,
    cwd: &Path,
) -> Result<TranscriptExportArgs, Box<dyn std::error::Error>> {
    let parts = split_shell_words(arg);
    let mut args = TranscriptExportArgs {
        min_turns: 2,
        require_tools: false,
        output: approved_training_data_path(),
        stats: false,
    };
    let mut index = 0;
    while index < parts.len() {
        let part = &parts[index];
        if part == "--stats" {
            args.stats = true;
        } else if part == "--tools" || part == "--require-tools" {
            args.require_tools = true;
        } else if part == "--min-turns" {
            index += 1;
            let value = parts
                .get(index)
                .ok_or("--min-turns requires a numeric value")?;
            args.min_turns = value.parse()?;
        } else if let Some(value) = part.strip_prefix("--min-turns=") {
            args.min_turns = value.parse()?;
        } else if part == "--output" {
            index += 1;
            let value = parts.get(index).ok_or("--output requires a path")?;
            args.output = resolve_cd_path(cwd, value);
        } else if let Some(value) = part.strip_prefix("--output=") {
            args.output = resolve_cd_path(cwd, value);
        } else {
            return Err(format!("unknown option: {part}").into());
        }
        index += 1;
    }
    Ok(args)
}

fn export_training_data(args: TranscriptExportArgs) -> Result<String, Box<dyn std::error::Error>> {
    let transcripts_dir = vibn_transcripts_dir();
    if !transcripts_dir.is_dir() {
        return Err(format!("No transcripts found at {}", transcripts_dir.display()).into());
    }

    let mut files = fs::read_dir(&transcripts_dir)?
        .filter_map(|entry| entry.ok().map(|value| value.path()))
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .collect::<Vec<_>>();
    files.sort();
    if files.is_empty() {
        return Err("No transcript files found.".into());
    }

    let mut total = 0;
    let mut exported = 0;
    let mut skipped_short = 0;
    let mut skipped_no_tools = 0;
    let mut skipped_bad = 0;
    let mut all_quality = Vec::new();
    let mut output_lines = Vec::new();

    for path in files {
        let messages = load_transcript_values(&path)?;
        total += 1;
        let score = score_transcript_session(&messages);
        all_quality.push(score.quality);

        if score.user_turns < args.min_turns {
            skipped_short += 1;
            continue;
        }
        if args.require_tools && score.tool_calls == 0 {
            skipped_no_tools += 1;
            continue;
        }
        if score.tool_calls > 0 && score.errors * 2 > score.tool_calls {
            skipped_bad += 1;
            continue;
        }

        let Some(example) = transcript_to_sharegpt(&messages) else {
            skipped_bad += 1;
            continue;
        };
        output_lines.push(serde_json::to_string(&example)?);
        exported += 1;
    }

    if args.stats {
        let mut lines = vec![
            "Transcript stats:".to_owned(),
            format!("  Total sessions:      {total}"),
            format!("  Would export:        {exported}"),
            format!("  Skipped (too short): {skipped_short}"),
            format!("  Skipped (no tools):  {skipped_no_tools}"),
            format!("  Skipped (too many errors): {skipped_bad}"),
        ];
        if !all_quality.is_empty() {
            let sum: isize = all_quality.iter().sum();
            let average = sum as f64 / all_quality.len() as f64;
            lines.push(format!("  Avg quality score:   {average:.1}"));
        }
        return Ok(lines.join("\n"));
    }

    if output_lines.is_empty() {
        return Err("Nothing to export after filtering.".into());
    }
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.output, format!("{}\n", output_lines.join("\n")))?;
    let size_kb = fs::metadata(&args.output)?.len() / 1024;

    Ok(format!(
        "Exported {exported}/{total} sessions -> {} ({size_kb} KB)\nSkipped: {skipped_short} too short, {skipped_no_tools} no tools, {skipped_bad} too many errors",
        args.output.display()
    ))
}

fn load_transcript_values(path: &Path) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut messages = Vec::new();
    for line in fs::read_to_string(path)?.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) == Some("metadata") {
            continue;
        }
        messages.push(value);
    }
    Ok(messages)
}

fn score_transcript_session(messages: &[Value]) -> TranscriptScore {
    let mut score = TranscriptScore {
        user_turns: 0,
        tool_calls: 0,
        tool_results: 0,
        assistant_turns: 0,
        errors: 0,
        quality: 0,
    };

    for message in messages {
        match message.get("role").and_then(Value::as_str).unwrap_or_default() {
            "user" => score.user_turns += 1,
            "assistant" => {
                score.assistant_turns += 1;
                score.tool_calls += message
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or_default();
            }
            "tool" => {
                score.tool_results += 1;
                let content = message
                    .get("content")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_lowercase();
                if content.starts_with("error") {
                    score.errors += 1;
                }
            }
            _ => {}
        }
    }
    score.quality = score.user_turns as isize * 2
        + score.tool_calls as isize * 3
        + score.tool_results as isize * 2
        - score.errors as isize * 2;
    score
}

fn transcript_to_sharegpt(messages: &[Value]) -> Option<Value> {
    let mut conversations = Vec::new();
    let mut index = 0;

    while index < messages.len() {
        let message = &messages[index];
        match message.get("role").and_then(Value::as_str).unwrap_or_default() {
            "system" => {
                conversations.push(json!({
                    "from": "system",
                    "value": message.get("content").and_then(Value::as_str).unwrap_or_default(),
                }));
                index += 1;
            }
            "user" => {
                conversations.push(json!({
                    "from": "human",
                    "value": message.get("content").and_then(Value::as_str).unwrap_or_default(),
                }));
                index += 1;
            }
            "assistant" => {
                let mut parts = Vec::new();
                if let Some(content) = message
                    .get("content")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    parts.push(content.to_owned());
                }
                if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
                    for tool_call in tool_calls {
                        let function = tool_call
                            .get("function")
                            .and_then(Value::as_object)
                            .cloned()
                            .unwrap_or_default();
                        let name = function
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let arguments = function
                            .get("arguments")
                            .map(normalize_tool_arguments)
                            .unwrap_or_else(|| Value::Object(Map::new()));
                        let payload = json!({
                            "name": name,
                            "arguments": arguments,
                        });
                        let rendered = serde_json::to_string_pretty(&payload)
                            .unwrap_or_else(|_| payload.to_string());
                        parts.push(format!("<tool_call>\n{rendered}\n</tool_call>"));
                    }
                }

                index += 1;
                while index < messages.len()
                    && messages[index].get("role").and_then(Value::as_str) == Some("tool")
                {
                    let tool_result = &messages[index];
                    let name = tool_result
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let content = tool_result
                        .get("content")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    parts.push(format!(
                        "<tool_result name=\"{name}\">\n{content}\n</tool_result>"
                    ));
                    index += 1;
                }

                let value = parts.join("\n\n");
                if !value.trim().is_empty() {
                    conversations.push(json!({
                        "from": "gpt",
                        "value": value,
                    }));
                }
            }
            _ => index += 1,
        }
    }

    let has_human = conversations.iter().any(|turn| {
        turn.get("from").and_then(Value::as_str) == Some("human")
    });
    let has_gpt = conversations
        .iter()
        .any(|turn| turn.get("from").and_then(Value::as_str) == Some("gpt"));
    if !has_human || !has_gpt {
        return None;
    }
    Some(json!({ "conversations": conversations }))
}

fn normalize_tool_arguments(value: &Value) -> Value {
    match value {
        Value::String(text) => serde_json::from_str::<Value>(text)
            .ok()
            .filter(Value::is_object)
            .unwrap_or_else(|| Value::Object(Map::new())),
        Value::Object(_) => value.clone(),
        _ => Value::Object(Map::new()),
    }
}

fn parse_training_generate_args(
    arg: &str,
    cwd: &Path,
) -> Result<TrainingGenerateArgs, Box<dyn std::error::Error>> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let mut args = TrainingGenerateArgs {
        cli: None,
        n: 2,
        timeout: Duration::from_secs(60),
        delay: Duration::from_secs(1),
        staging: staging_dir_path().join(format!("generated-{timestamp}.jsonl")),
        dry_run: false,
    };
    let parts = split_shell_words(arg);
    let mut index = 0;
    while index < parts.len() {
        let part = &parts[index];
        if part == "--dry-run" {
            args.dry_run = true;
        } else if part == "--cli" {
            index += 1;
            let value = parts.get(index).ok_or("--cli requires a value")?;
            validate_training_cli(value)?;
            args.cli = Some(value.clone());
        } else if let Some(value) = part.strip_prefix("--cli=") {
            validate_training_cli(value)?;
            args.cli = Some(value.to_owned());
        } else if part == "--n" {
            index += 1;
            args.n = parts.get(index).ok_or("--n requires a number")?.parse()?;
        } else if let Some(value) = part.strip_prefix("--n=") {
            args.n = value.parse()?;
        } else if part == "--timeout" {
            index += 1;
            let seconds: u64 = parts.get(index).ok_or("--timeout requires seconds")?.parse()?;
            args.timeout = Duration::from_secs(seconds);
        } else if let Some(value) = part.strip_prefix("--timeout=") {
            args.timeout = Duration::from_secs(value.parse()?);
        } else if part == "--delay" {
            index += 1;
            let seconds: f64 = parts.get(index).ok_or("--delay requires seconds")?.parse()?;
            args.delay = Duration::from_secs_f64(seconds.max(0.0));
        } else if let Some(value) = part.strip_prefix("--delay=") {
            let seconds: f64 = value.parse()?;
            args.delay = Duration::from_secs_f64(seconds.max(0.0));
        } else if part == "--staging" {
            index += 1;
            let value = parts.get(index).ok_or("--staging requires a path")?;
            args.staging = resolve_cd_path(cwd, value);
        } else if let Some(value) = part.strip_prefix("--staging=") {
            args.staging = resolve_cd_path(cwd, value);
        } else {
            return Err(format!("unknown option: {part}").into());
        }
        index += 1;
    }
    Ok(args)
}

fn validate_training_cli(value: &str) -> Result<(), Box<dyn std::error::Error>> {
    match value {
        "claude" | "codex" | "copilot" => Ok(()),
        _ => Err(format!("unknown CLI: {value}").into()),
    }
}

fn spawn_training_generate_task(args: TrainingGenerateArgs, cwd: PathBuf, tx: Sender<UiEvent>) {
    thread::spawn(move || {
        let result = generate_training_data(args, &cwd, &tx);
        let _ = tx.send(UiEvent::Append(
            result.unwrap_or_else(|error| format!("Error: {error}")),
        ));
        let _ = tx.send(UiEvent::SetProcessing(false));
    });
}

fn generate_training_data(
    args: TrainingGenerateArgs,
    cwd: &Path,
    tx: &Sender<UiEvent>,
) -> Result<String, Box<dyn std::error::Error>> {
    let seeds = load_training_seeds()?;
    let constraints = load_training_constraints()?;
    let existing_components = scan_existing_components(cwd, &constraints);
    let system = build_training_system_prompt(&seeds, &constraints, &existing_components);
    let prompts = build_training_prompts(&seeds, args.n)?;
    let clis = args
        .cli
        .clone()
        .map(|cli| vec![cli])
        .unwrap_or_else(|| vec!["claude".to_owned(), "codex".to_owned(), "copilot".to_owned()]);

    if let Some(parent) = args.staging.parent() {
        fs::create_dir_all(parent)?;
    }

    let total = prompts.len() * clis.len();
    let mut passed = 0;
    let mut failed = 0;
    let mut errors = 0;
    let mut index = 0;

    let _ = tx.send(UiEvent::Append(format!(
        "Generating {total} examples ({} prompts x {} CLIs)\nOutput: {}",
        prompts.len(),
        clis.len(),
        args.staging.display()
    )));

    for prompt in prompts {
        for cli in &clis {
            index += 1;
            let prefix = format!(
                "[{index:>3}/{total}] {cli:<8} {}: {}...",
                prompt.component,
                prompt.prompt.chars().take(60).collect::<String>()
            );
            if args.dry_run {
                let _ = tx.send(UiEvent::Append(format!("{prefix} (dry run)")));
                continue;
            }

            let response = match call_training_cli(cli, &system, &prompt.prompt, args.timeout) {
                Ok(Some(response)) => response,
                Ok(None) => {
                    errors += 1;
                    let _ = tx.send(UiEvent::Append(format!("{prefix} ERROR (no response)")));
                    continue;
                }
                Err(error) => {
                    errors += 1;
                    let _ = tx.send(UiEvent::Append(format!("{prefix} ERROR ({error})")));
                    continue;
                }
            };

            let code = extract_training_code(&response);
            let validation = validate_training_code(&code);
            if validation.passed {
                let mut example = training_response_to_sharegpt(&system, &prompt.prompt, &response);
                if let Some(object) = example.as_object_mut() {
                    object.insert(
                        "_meta".to_owned(),
                        json!({
                            "source": cli,
                            "component": prompt.component,
                            "score": validation.score,
                            "max_score": validation.max_score,
                            "generated_at": SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs().to_string(),
                            "reviewed": false,
                        }),
                    );
                }
                append_jsonl_line(&args.staging, &example)?;
                passed += 1;
                let _ = tx.send(UiEvent::Append(format!(
                    "{prefix} ok score={}/{}",
                    validation.score, validation.max_score
                )));
            } else {
                failed += 1;
                let _ = tx.send(UiEvent::Append(format!(
                    "{prefix} fail {}",
                    format_training_validation(&validation)
                )));
            }

            if !args.delay.is_zero() {
                thread::sleep(args.delay);
            }
        }
    }

    Ok(format!(
        "Done. {passed} passed, {failed} failed validation, {errors} errors\nStaging file: {} ({} KB)\nNext step: run /review-training in Vibn to approve examples",
        args.staging.display(),
        fs::metadata(&args.staging).map(|value| value.len() / 1024).unwrap_or(0)
    ))
}

fn load_training_seeds() -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(&fs::read_to_string(
        repo_root().join("training/seeds.json"),
    )?)?)
}

fn load_training_constraints() -> Result<Value, Box<dyn std::error::Error>> {
    let path = constraints_file_path();
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn build_training_system_prompt(
    seeds: &Value,
    constraints: &Value,
    existing_components: &[String],
) -> String {
    let mut rules = string_array(seeds.get("rules"));
    rules.extend(string_array(
        constraints.get("hard_rules").or_else(|| constraints.get("hardRules")),
    ));
    rules.extend(string_array(
        constraints.get("soft_rules").or_else(|| constraints.get("softRules")),
    ));
    let rules_text = rules
        .iter()
        .map(|rule| format!("- {rule}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut prompt = format!(
        "{}\n\nStrict rules:\n{}",
        seeds
            .get("system_prompt")
            .and_then(Value::as_str)
            .unwrap_or("You are a senior full-stack engineer."),
        rules_text
    );
    if !existing_components.is_empty() {
        let message = constraints
            .get("no_duplicate")
            .and_then(|value| value.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("Reuse existing components rather than creating duplicates.");
        prompt.push_str(&format!(
            "\n\nExisting project components (DO NOT recreate these; import and reuse them):\n{}\n\n{}",
            existing_components.join(", "),
            message
        ));
    }
    prompt
}

fn build_training_prompts(
    seeds: &Value,
    n_per_template: usize,
) -> Result<Vec<TrainingPrompt>, Box<dyn std::error::Error>> {
    let templates = seeds
        .get("prompt_templates")
        .and_then(Value::as_array)
        .ok_or("training seeds missing prompt_templates")?;
    let components = seeds
        .get("components")
        .and_then(Value::as_array)
        .ok_or("training seeds missing components")?;
    let api_routes = seeds
        .get("api_routes")
        .and_then(Value::as_array)
        .ok_or("training seeds missing api_routes")?;
    let refactors = seeds
        .get("refactor_tasks")
        .and_then(Value::as_array)
        .ok_or("training seeds missing refactor_tasks")?;
    let entities = ["User", "Post", "Product", "Order", "Comment", "Project"];
    let mut prompts = Vec::new();

    for round in 0..n_per_template {
        for (template_index, template) in templates.iter().enumerate() {
            let template = template.as_str().unwrap_or_default();
            let component = &components[(round + template_index) % components.len()];
            let route = &api_routes[(round + template_index) % api_routes.len()];
            let refactor = refactors[(round + template_index) % refactors.len()]
                .as_str()
                .unwrap_or_default();
            let entity = entities[(round + template_index) % entities.len()];
            let component_name = component
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("Component");
            let description = if template.contains("{path}") {
                route
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            } else {
                component
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            };
            let prompt = template
                .replace("{component}", component_name)
                .replace("{description}", description)
                .replace(
                    "{path}",
                    route
                        .get("path")
                        .and_then(Value::as_str)
                        .unwrap_or("app/api/route.ts"),
                )
                .replace("{refactor_task}", refactor)
                .replace("{entity}", entity);
            prompts.push(TrainingPrompt {
                prompt,
                component: component_name.to_owned(),
            });
        }
    }
    Ok(prompts)
}

fn scan_existing_components(project_dir: &Path, constraints: &Value) -> Vec<String> {
    let no_duplicate = constraints.get("no_duplicate").unwrap_or(&Value::Null);
    let scan_dirs = no_duplicate
        .get("scan_dirs")
        .map(|value| string_array(Some(value)))
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec!["components".to_owned()]);
    let patterns = no_duplicate
        .get("file_patterns")
        .map(|value| string_array(Some(value)))
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec!["*.tsx".to_owned(), "*.ts".to_owned()]);
    let mut found = BTreeSet::new();
    for scan_dir in scan_dirs {
        let target = project_dir.join(scan_dir);
        collect_component_names(&target, &patterns, &mut found);
    }
    found.into_iter().collect()
}

fn collect_component_names(path: &Path, patterns: &[String], found: &mut BTreeSet<String>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_component_names(&path, patterns, found);
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !patterns.iter().any(|pattern| glob_matches_name(pattern, file_name)) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if stem.chars().next().is_some_and(char::is_uppercase) {
            found.insert(stem.to_owned());
        }
    }
}

fn glob_matches_name(pattern: &str, file_name: &str) -> bool {
    pattern == file_name
        || pattern
            .strip_prefix("*")
            .is_some_and(|suffix| file_name.ends_with(suffix))
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn call_training_cli(
    cli: &str,
    system: &str,
    prompt: &str,
    timeout: Duration,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let full_prompt = format!("{system}\n\n{prompt}");
    let mut command = match cli {
        "claude" => {
            let mut command = Command::new("claude");
            command.args(["-p", &full_prompt, "--dangerously-skip-permissions"]);
            command
        }
        "codex" => {
            let mut command = Command::new("codex");
            command.args(["exec", "--quiet", &full_prompt]);
            command.env("CODEX_QUIET", "1");
            command
        }
        "copilot" => {
            let mut command = Command::new("copilot");
            command.args(["-p", &full_prompt]);
            command
        }
        _ => return Err(format!("unknown CLI: {cli}").into()),
    };
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let started = SystemTime::now();
    loop {
        if child.try_wait()?.is_some() {
            let output = child.wait_with_output()?;
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if stdout.is_empty() {
                return Ok(None);
            }
            return Ok(Some(stdout));
        }
        if started.elapsed().unwrap_or_default() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn extract_training_code(response: &str) -> String {
    for language in ["tsx", "typescript", "ts", "jsx", ""] {
        let pattern = if language.is_empty() {
            r"(?s)```\n(.*?)```".to_owned()
        } else {
            format!(r"(?s)```{}\n(.*?)```", regex::escape(language))
        };
        let Ok(regex) = Regex::new(&pattern) else {
            continue;
        };
        let matches = regex
            .captures_iter(response)
            .filter_map(|captures| captures.get(1).map(|value| value.as_str().trim().to_owned()))
            .collect::<Vec<_>>();
        if let Some(longest) = matches.into_iter().max_by_key(String::len) {
            return longest;
        }
    }
    response.trim().to_owned()
}

fn training_response_to_sharegpt(system: &str, user_prompt: &str, response: &str) -> Value {
    json!({
        "conversations": [
            { "from": "system", "value": system },
            { "from": "human", "value": user_prompt },
            { "from": "gpt", "value": response },
        ]
    })
}

fn validate_training_code(code: &str) -> ValidationResult {
    let validators = [
        ("tailwind", validate_tailwind(code), 3usize),
        ("shadcn", validate_shadcn(code), 2),
        ("typescript", validate_typescript(code), 2),
        ("no_raw_sql", validate_no_raw_sql(code), 2),
        ("no_manual_auth", validate_no_manual_auth(code), 2),
        ("cn_usage", validate_cn_usage(code), 1),
    ];
    let max_score = validators.iter().map(|(_, _, weight)| *weight).sum::<usize>();
    let mut score = 0;
    let mut results = Vec::new();
    for (name, (passed, reason), weight) in validators {
        if passed {
            score += weight;
        }
        results.push((name.to_owned(), passed, reason));
    }
    let tailwind_passed = results
        .iter()
        .find(|(name, _, _)| name == "tailwind")
        .map(|(_, passed, _)| *passed)
        .unwrap_or(false);
    ValidationResult {
        passed: tailwind_passed && score * 10 >= max_score * 6,
        score,
        max_score,
        results,
    }
}

fn validate_tailwind(code: &str) -> (bool, String) {
    if regex_match(r"style\s*=\s*\{", code) {
        return (false, "uses style={} inline styles".to_owned());
    }
    if code.contains("<style") && !code.contains("@layer") {
        return (false, "uses <style> tags".to_owned());
    }
    let css_import = Regex::new(r#"import\s+['"]([^'"]+\.css)['"]"#).expect("regex");
    let bad_css = css_import
        .captures_iter(code)
        .filter_map(|captures| captures.get(1).map(|value| value.as_str().to_owned()))
        .filter(|path| !path.contains("globals"))
        .collect::<Vec<_>>();
    if !bad_css.is_empty() {
        return (false, format!("imports CSS files: {bad_css:?}"));
    }
    let arbitrary = Regex::new(r"\[(?:[\d.]+(?:px|rem|em|vh|vw|%)|#[0-9a-fA-F]{3,6})\]")
        .expect("regex")
        .find_iter(code)
        .map(|value| value.as_str().to_owned())
        .collect::<Vec<_>>();
    if arbitrary.len() > 2 {
        return (
            false,
            format!(
                "too many arbitrary Tailwind values: {:?}",
                arbitrary.into_iter().take(3).collect::<Vec<_>>()
            ),
        );
    }
    (true, "ok".to_owned())
}

fn validate_shadcn(code: &str) -> (bool, String) {
    let primitives = [
        "Button",
        "Card",
        "CardHeader",
        "CardContent",
        "CardFooter",
        "Input",
        "Label",
        "Badge",
        "Avatar",
        "Dialog",
        "Sheet",
        "Tabs",
        "Select",
        "DropdownMenu",
        "Popover",
        "Separator",
        "Skeleton",
        "Table",
        "Form",
        "Command",
        "Toast",
    ];
    if primitives.iter().any(|primitive| code.contains(primitive)) {
        return (true, "ok".to_owned());
    }
    for pattern in [
        r"(?i)<button(?:\s|>)",
        r"(?i)<input(?:\s|>)",
        r"(?i)<select(?:\s|>)",
        r"(?i)<textarea(?:\s|>)",
    ] {
        if regex_match(pattern, code) {
            return (
                false,
                "uses raw HTML UI elements without shadcn primitives".to_owned(),
            );
        }
    }
    (true, "ok (non-UI file)".to_owned())
}

fn validate_typescript(code: &str) -> (bool, String) {
    if code.trim().is_empty() {
        return (false, "empty".to_owned());
    }
    let has_types = [
        ": string",
        ": number",
        ": boolean",
        "interface ",
        "type ",
        ": React.",
        "FC<",
        "Promise<",
        ": void",
        ": JSX",
    ]
    .iter()
    .any(|needle| code.contains(needle));
    if regex_match(r":\s*any\b", code) {
        return (false, "uses 'any' type".to_owned());
    }
    if !has_types && (code.contains(".tsx") || code.contains("React")) {
        return (false, "missing TypeScript type annotations".to_owned());
    }
    (true, "ok".to_owned())
}

fn validate_no_raw_sql(code: &str) -> (bool, String) {
    for pattern in [
        r"(?i)`\s*SELECT\s+",
        r"(?i)`\s*INSERT\s+INTO",
        r"(?i)`\s*UPDATE\s+\w+\s+SET",
        r"(?i)`\s*DELETE\s+FROM",
        r"(?i)pg\.query\(",
        r"(?i)mysql\.query\(",
        r#"(?i)import.*from\s+['"]pg['"]"#,
        r#"(?i)import.*from\s+['"]mysql"#,
    ] {
        if regex_match(pattern, code) {
            return (false, "uses raw SQL or direct DB driver".to_owned());
        }
    }
    (true, "ok".to_owned())
}

fn validate_no_manual_auth(code: &str) -> (bool, String) {
    for pattern in [
        r"(?i)jwt\.sign\(",
        r"(?i)jwt\.verify\(",
        r"(?i)bcrypt\.hash\(",
        r"(?i)bcrypt\.compare\(",
        r"(?i)createSession\(",
        r"(?i)iron-session",
        r"(?i)next-auth",
        r"(?i)import.*jsonwebtoken",
    ] {
        if regex_match(pattern, code) {
            return (
                false,
                "implements manual auth instead of using better-auth".to_owned(),
            );
        }
    }
    (true, "ok".to_owned())
}

fn validate_cn_usage(code: &str) -> (bool, String) {
    if regex_match(r"`[^`]*(?:px-|py-|text-|bg-|flex|grid|rounded)[^`]*\$\{", code) {
        return (
            false,
            "uses template literal string concatenation for Tailwind classes (use cn() instead)"
                .to_owned(),
        );
    }
    (true, "ok".to_owned())
}

fn regex_match(pattern: &str, code: &str) -> bool {
    Regex::new(pattern)
        .map(|regex| regex.is_match(code))
        .unwrap_or(false)
}

fn format_training_validation(validation: &ValidationResult) -> String {
    let parts = validation
        .results
        .iter()
        .map(|(name, passed, reason)| {
            if *passed {
                format!("ok:{name}")
            } else {
                format!("fail:{name}({reason})")
            }
        })
        .collect::<Vec<_>>()
        .join("  ");
    let status = if validation.passed { "PASS" } else { "FAIL" };
    format!(
        "[{status}] {}/{}  {parts}",
        validation.score, validation.max_score
    )
}

fn load_prompts() -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let path = prompts_file_path();
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn save_prompts(prompts: &BTreeMap<String, String>) -> Result<(), Box<dyn std::error::Error>> {
    let path = prompts_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(prompts)?)?;
    Ok(())
}

fn prompt_commit_confirmation(message: &str) -> Result<String, Box<dyn std::error::Error>> {
    with_terminal_suspended(|| {
        println!("\nProposed commit message:\n{message}\n");
        print!("Commit with this message? [Y/n/edit]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Ok(input.trim().to_lowercase())
    })
}

fn parse_compare_command(arg: &str, current_model: &str) -> Option<(String, String)> {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut model_b = if current_model.contains("7b") {
        "qwen2.5-coder:14b".to_owned()
    } else {
        "qwen2.5-coder:7b".to_owned()
    };
    let mut prompt = trimmed.to_owned();
    if let Some(first) = trimmed.split_whitespace().next() {
        if first.contains(':') && !first.contains('/') {
            model_b = first.trim_end_matches(':').to_owned();
            prompt = trimmed[first.len()..].trim().to_owned();
        }
    }
    if prompt.is_empty() {
        None
    } else {
        Some((model_b, prompt))
    }
}

fn parse_watch_command(arg: &str) -> Option<(String, String)> {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let path = parts.next()?.trim();
    if path.is_empty() {
        return None;
    }
    let prompt = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Files changed in {path}. Review and handle the changes.");
    Some((path.to_owned(), prompt.to_owned()))
}

fn collect_watch_snapshot(path: &Path) -> BTreeMap<PathBuf, SystemTime> {
    fn walk(current: &Path, snapshot: &mut BTreeMap<PathBuf, SystemTime>) {
        let Ok(read_dir) = fs::read_dir(current) else {
            return;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, snapshot);
            } else if let Ok(metadata) = fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    snapshot.insert(path, modified);
                }
            }
        }
    }

    let mut snapshot = BTreeMap::new();
    if path.is_dir() {
        walk(path, &mut snapshot);
    } else if let Ok(metadata) = fs::metadata(path) {
        if let Ok(modified) = metadata.modified() {
            snapshot.insert(path.to_path_buf(), modified);
        }
    }
    snapshot
}

fn remember_history_entry(state: &mut UiState, entry: &str) {
    if entry.is_empty() {
        return;
    }
    if state.input_history.last().is_none_or(|last| last != entry) {
        state.input_history.push(entry.to_owned());
        let path = history_file_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut content = state.input_history.join("\n");
        content.push('\n');
        let _ = fs::write(path, content);
    }
    state.history_index = None;
    state.history_draft.clear();
}

fn history_backward(input: &mut TextArea<'static>, state: &mut UiState) {
    if state.input_history.is_empty() {
        return;
    }
    if state.history_index.is_none() {
        state.history_draft = current_input_text(input);
        state.history_index = Some(state.input_history.len() - 1);
    } else if let Some(index) = state.history_index.as_mut() {
        *index = index.saturating_sub(1);
    }
    if let Some(index) = state.history_index {
        set_input_text(input, &state.input_history[index]);
    }
}

fn history_forward(input: &mut TextArea<'static>, state: &mut UiState) {
    let Some(index) = state.history_index else {
        return;
    };
    if index + 1 >= state.input_history.len() {
        state.history_index = None;
        set_input_text(input, &state.history_draft);
        return;
    }
    state.history_index = Some(index + 1);
    set_input_text(input, &state.input_history[index + 1]);
}

fn set_input_text(input: &mut TextArea<'static>, text: &str) {
    *input = build_input();
    if !text.is_empty() {
        input.insert_str(text);
    }
}

fn pending_modal_response(key: KeyEvent, state: &mut UiState) -> Option<InputAction> {
    if state.pending_diff.is_some() {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(pending) = state.pending_diff.take() {
                    let _ = pending.reply.send(true);
                }
                return Some(InputAction::Continue);
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                if let Some(pending) = state.pending_diff.take() {
                    let _ = pending.reply.send(false);
                }
                return Some(InputAction::Continue);
            }
            _ => return Some(InputAction::Continue),
        }
    }

    if state.pending_confirm.is_some() {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(pending) = state.pending_confirm.take() {
                    let _ = pending.reply.send(true);
                }
                return Some(InputAction::Continue);
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                if let Some(pending) = state.pending_confirm.take() {
                    let _ = pending.reply.send(false);
                }
                return Some(InputAction::Continue);
            }
            _ => return Some(InputAction::Continue),
        }
    }

    None
}

fn execute_slash_command(
    input: &mut TextArea<'static>,
    state: &mut UiState,
    command_text: &str,
    tx: Sender<UiEvent>,
) -> Result<InputAction, Box<dyn std::error::Error>> {
    if command_text.trim() == "/" {
        state.append("Type a command after /. Press Tab or Up/Down to choose a suggestion.");
        return Ok(InputAction::Continue);
    }

    state.config.track_command_usage(command_text);

    let mut parts = command_text.splitn(2, char::is_whitespace);
    let command = parts.next().unwrap_or("");
    let arg = parts.next().unwrap_or("").trim();

    let action = match command {
        "/quit" | "/exit" | "/q" => InputAction::Quit,
        "/help" => {
            let lines = slash_command_definitions()
                .iter()
                .map(|definition| {
                    format!(
                        "{:<24} {}",
                        definition.command.trim_end(),
                        definition.description
                    )
                })
                .collect::<Vec<_>>();
            state.append(lines.join("\n"));
            InputAction::Continue
        }
        "/status" => {
            let mcp_summary = connected_mcp_summary();
            state.append(format!(
                "model: {}\nsession: {}\ncwd: {}\ncommand timeout: {}s\nplan mode: {}\n{}",
                state.model,
                state.session_id,
                state.cwd.display(),
                state.config.command_timeout_secs(),
                if state.plan_mode { "on" } else { "off" },
                if mcp_summary.is_empty() {
                    "mcp: none connected".to_owned()
                } else {
                    format!("mcp:\n{}", mcp_summary)
                }
            ));
            InputAction::Continue
        }
        "/market" | "/marketplace" => {
            if arg.is_empty() {
                open_browser_mode(state, input, BrowserView::MarketTop, None);
            } else if let Some(query) = arg.strip_prefix("search ") {
                let query = query.trim();
                if query.is_empty() {
                    state.append("Usage: /market search <query>");
                } else {
                    open_browser_mode(
                        state,
                        input,
                        BrowserView::MarketSearch(query.to_owned()),
                        None,
                    );
                }
            } else if let Some(name) = arg.strip_prefix("install ") {
                match start_marketplace_install(state, input, name.trim()) {
                    Ok(message) if !message.is_empty() => state.append(message),
                    Ok(_) => {}
                    Err(error) => state.append(format!("Error: {error}")),
                }
            } else if arg == "all" {
                open_browser_mode(state, input, BrowserView::MarketAll, None);
            } else if MARKET_CATEGORIES.iter().any(|(key, _)| *key == arg) {
                open_browser_mode(
                    state,
                    input,
                    BrowserView::MarketCategory(arg.to_owned()),
                    None,
                );
            } else {
                state.append(format!("Unknown marketplace category: {arg}"));
            }
            InputAction::Continue
        }
        "/skills" | "/skill-list" => {
            open_browser_mode(state, input, BrowserView::Skills, None);
            InputAction::Continue
        }
        "/skill" => {
            if arg.is_empty() {
                open_browser_mode(state, input, BrowserView::Skills, None);
            } else {
                match activate_skill_into_session(state, arg) {
                    Ok(message) => state.append(message),
                    Err(error) => state.append(format!("Error: {error}")),
                }
            }
            InputAction::Continue
        }
        "/mcp" => {
            let mut subparts = arg.splitn(2, char::is_whitespace);
            let subcmd = subparts.next().unwrap_or("").trim();
            let subarg = subparts.next().unwrap_or("").trim();
            match subcmd {
                "" => {
                    open_browser_mode(state, input, BrowserView::McpManager, None);
                }
                "list" => {
                    let connected = list_connected_mcp_servers().unwrap_or_default();
                    let configured = configured_mcp_servers(&state.config);
                    if configured.is_empty() && connected.is_empty() {
                        state.append("No MCP servers configured.");
                    } else {
                        let mut lines = Vec::new();
                        for status in &connected {
                            let suffix = if status.args.is_empty() {
                                status.command.clone()
                            } else {
                                format!("{} {}", status.command, status.args.join(" "))
                            };
                            lines.push(format!(
                                "[connected] {} — {} tool(s) — {}",
                                status.name, status.tool_count, suffix
                            ));
                        }
                        let mut configured_names = configured.keys().cloned().collect::<Vec<_>>();
                        configured_names.sort();
                        for name in configured_names {
                            if connected.iter().any(|status| status.name == name) {
                                continue;
                            }
                            let server = configured
                                .get(&name)
                                .and_then(Value::as_object)
                                .cloned()
                                .unwrap_or_default();
                            let command = server
                                .get("command")
                                .and_then(Value::as_str)
                                .unwrap_or("<missing command>");
                            let args = server
                                .get("args")
                                .and_then(Value::as_array)
                                .map(|items| {
                                    items
                                        .iter()
                                        .filter_map(Value::as_str)
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                })
                                .unwrap_or_default();
                            let suffix = if args.is_empty() {
                                command.to_owned()
                            } else {
                                format!("{command} {args}")
                            };
                            lines.push(format!("[configured] {} — {}", name, suffix));
                        }
                        state.append(lines.join("\n"));
                    }
                }
                "connect" => {
                    let servers = state
                        .config
                        .extra
                        .get("mcp_servers")
                        .and_then(Value::as_object)
                        .cloned()
                        .unwrap_or_default();
                    if servers.is_empty() {
                        state.append("No MCP servers configured. Add one with /mcp add <name> <command> [args...]");
                    } else if subarg.is_empty() {
                        let mut results = Vec::new();
                        let mut names = servers.keys().cloned().collect::<Vec<_>>();
                        names.sort();
                        for name in names {
                            let Some(server) = servers.get(&name).and_then(Value::as_object) else {
                                continue;
                            };
                            let Some(command) = server.get("command").and_then(Value::as_str)
                            else {
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
                            match connect_mcp_server(&name, command, &args, &env_vars) {
                                Ok(tool_count) => results.push(format!(
                                    "Connected {} ({} tool{})",
                                    name,
                                    tool_count,
                                    if tool_count == 1 { "" } else { "s" }
                                )),
                                Err(error) => results.push(format!("{}: {}", name, error)),
                            }
                        }
                        state.append(results.join("\n"));
                    } else if let Some(server) = servers.get(subarg).and_then(Value::as_object) {
                        let Some(command) = server.get("command").and_then(Value::as_str) else {
                            state.append(format!("Missing command for {}", subarg));
                            return Ok(InputAction::Continue);
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
                        match connect_mcp_server(subarg, command, &args, &env_vars) {
                            Ok(tool_count) => state.append(format!(
                                "Connected {} ({} tool{})",
                                subarg,
                                tool_count,
                                if tool_count == 1 { "" } else { "s" }
                            )),
                            Err(error) => state.append(format!("Error: {error}")),
                        }
                    } else {
                        state.append(format!("Unknown MCP server: {subarg}"));
                    }
                }
                "disconnect" => {
                    if subarg.is_empty() {
                        state.append("Usage: /mcp disconnect <name>|all");
                    } else if subarg == "all" {
                        let connected = list_connected_mcp_servers().unwrap_or_default();
                        if connected.is_empty() {
                            state.append("No MCP servers are connected.");
                        } else {
                            let mut results = Vec::new();
                            for status in connected {
                                match disconnect_mcp_server(&status.name) {
                                    Ok(()) => results.push(format!("Disconnected {}", status.name)),
                                    Err(error) => {
                                        results.push(format!("{}: {}", status.name, error))
                                    }
                                }
                            }
                            state.append(results.join("\n"));
                        }
                    } else {
                        match disconnect_mcp_server(subarg) {
                            Ok(()) => state.append(format!("Disconnected {subarg}")),
                            Err(error) => state.append(format!("Error: {error}")),
                        }
                    }
                }
                "add" => {
                    let parts = split_shell_words(subarg);
                    if parts.len() < 2 {
                        state.append("Usage: /mcp add <name> <command> [args...]");
                    } else {
                        let name = parts[0].clone();
                        let command = parts[1].clone();
                        let args = parts[2..]
                            .iter()
                            .map(|value| Value::String(value.clone()))
                            .collect::<Vec<_>>();
                        let mut servers = state
                            .config
                            .extra
                            .get("mcp_servers")
                            .and_then(Value::as_object)
                            .cloned()
                            .unwrap_or_default();
                        servers.insert(
                            name.clone(),
                            json!({
                                "command": command,
                                "args": args,
                            }),
                        );
                        state
                            .config
                            .extra
                            .insert("mcp_servers".to_owned(), Value::Object(servers));
                        save_config(&state.config)?;
                        state.append(format!(
                            "Saved MCP server {name}. Connect with /mcp connect {name}"
                        ));
                    }
                }
                "remove" => {
                    if subarg.is_empty() {
                        state.append("Usage: /mcp remove <name>");
                    } else {
                        let Some(servers) = state
                            .config
                            .extra
                            .get_mut("mcp_servers")
                            .and_then(Value::as_object_mut)
                        else {
                            state.append("No MCP servers configured.");
                            return Ok(InputAction::Continue);
                        };
                        if servers.remove(subarg).is_some() {
                            let _ = disconnect_mcp_server(subarg);
                            save_config(&state.config)?;
                            state.append(format!("Removed MCP server {subarg}"));
                        } else {
                            state.append(format!("Unknown MCP server: {subarg}"));
                        }
                    }
                }
                _ => state.append("Usage: /mcp [list|connect|disconnect|add|remove]"),
            }
            InputAction::Continue
        }
        "/constraints" => {
            state.constraints_mode.rules = load_constraints()?;
            state.constraints_mode.selected = state
                .constraints_mode
                .selected
                .min(state.constraints_mode.rules.len().saturating_sub(1));
            state.constraints_mode.edit_index = state.constraints_mode.selected;
            state.constraints_mode.active = true;
            state.constraints_mode.adding = false;
            state.constraints_mode.editing = false;
            *input = build_input();
            InputAction::Continue
        }
        "/review-training" => {
            if state.processing || state.pending_confirm.is_some() || state.pending_diff.is_some() {
                state.append("Finish the current operation before starting review mode.");
            } else if let Some(staging_file) = latest_staging_file() {
                let examples = load_review_examples(&staging_file)?;
                if examples.is_empty() {
                    state.append("Staging file is empty.");
                } else {
                    let unreviewed = examples
                        .iter()
                        .filter(|example| !example_reviewed(example))
                        .count();
                    state.append(format!(
                        "Loaded {} examples ({} unreviewed) from {}",
                        examples.len(),
                        unreviewed,
                        staging_file.display()
                    ));
                    state.review_mode.active = true;
                    state.review_mode.examples = examples;
                    state.review_mode.index = first_unreviewed_index(&state.review_mode.examples);
                    state.review_mode.staging_file = staging_file.display().to_string();
                    state.review_mode.approved = 0;
                    state.review_mode.discarded = 0;
                    state.review_mode.skipped = 0;
                    state.auto_scroll = false;
                    state.scroll_offset = 0;
                    *input = build_input();
                }
            } else {
                state.append("No staging files found.\nRun /generate-training-data first.");
            }
            InputAction::Continue
        }
        "/resume" => {
            let entries = list_transcripts(10)?;
            if entries.is_empty() {
                state.append("No transcripts found.");
                InputAction::Continue
            } else if arg.is_empty() {
                let mut lines = entries
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        format!(
                            "{}. {}  {} msgs · {}",
                            index + 1,
                            entry.session_id,
                            entry.messages,
                            entry.timestamp.chars().take(16).collect::<String>()
                        )
                    })
                    .collect::<Vec<_>>();
                lines.push("  /resume <number>".to_owned());
                state.append(lines.join("\n"));
                InputAction::Continue
            } else {
                let sid = if let Ok(index) = arg.parse::<usize>() {
                    if index > 0 && index <= entries.len() {
                        entries[index - 1].session_id.clone()
                    } else {
                        arg.to_owned()
                    }
                } else {
                    arg.to_owned()
                };
                let (metadata, messages) = load_transcript(&sid)?;
                if messages.is_empty() {
                    state.append(format!("Not found: {sid}"));
                    InputAction::Continue
                } else {
                    if let Some(model) = metadata
                        .as_ref()
                        .and_then(|meta| meta.extra.get("model"))
                        .and_then(Value::as_str)
                    {
                        state.model = model.to_owned();
                    }
                    state.session_id = sid.clone();
                    state.output_lines = render_messages(&messages);
                    state.scroll_offset = 0;
                    state.auto_scroll = true;
                    state.append(format!("Resumed {sid} ({} messages)", messages.len()));
                    InputAction::Continue
                }
            }
        }
        "/reset" => {
            save_session_messages(state, &[])?;
            state.reset_output();
            state.append("Conversation reset.");
            InputAction::Continue
        }
        "/undo" => {
            let (_, messages) = load_transcript(&state.session_id)?;
            if let Some(trimmed) = truncate_last_turn(&messages) {
                let removed = messages.len().saturating_sub(trimmed.len());
                save_session_messages(state, &trimmed)?;
                state.output_lines = render_messages(&trimmed);
                state.scroll_offset = 0;
                state.auto_scroll = true;
                state.append(format!("Undone — removed last turn ({removed} messages)"));
            } else {
                state.append("Nothing to undo.");
            }
            InputAction::Continue
        }
        "/cd" => {
            if arg.is_empty() {
                state.append(state.cwd.display().to_string());
            } else {
                let next = resolve_cd_path(&state.cwd, arg);
                if next.is_dir() {
                    env::set_current_dir(&next)?;
                    state.cwd = next.canonicalize().unwrap_or(next);
                    state.apply_project_config();
                    state.append(format!("Changed directory to {}", state.cwd.display()));
                } else {
                    state.append(format!("Directory not found: {arg}"));
                }
            }
            InputAction::Continue
        }
        "/model" => {
            if matches!(arg, "check" | "perf" | "recommend" | "recommendations") {
                open_browser_mode(state, input, BrowserView::ModelPicker, None);
            } else if arg.is_empty() {
                open_browser_mode(state, input, BrowserView::ModelPicker, None);
            } else {
                match switch_model(state, arg) {
                    Ok(message) => state.append(message),
                    Err(error) => state.append(format!("Error: {error}")),
                }
            }
            InputAction::Continue
        }
        "/model-path" => {
            if arg.is_empty() {
                open_browser_mode(state, input, BrowserView::StoragePicker, None);
            } else {
                match apply_model_path_change(&mut state.config, &state.cwd, arg) {
                    Ok(message) => state.append(message),
                    Err(error) => state.append(format!("Error: {error}")),
                }
            }
            InputAction::Continue
        }
        "/models" | "/perf" | "/model-check" => {
            open_browser_mode(state, input, BrowserView::ModelPicker, None);
            InputAction::Continue
        }
        "/vision-model" => set_config_string_field(state, "vision_model", arg, "vision model"),
        "/image-model" => set_config_string_field(state, "image_gen_model", arg, "image-gen model"),
        "/video-model" => set_config_string_field(state, "video_gen_model", arg, "video-gen model"),
        "/comfy-url" => set_config_string_field(state, "comfyui_url", arg, "ComfyUI URL"),
        "/install-comfy" => {
            state.append("Installing managed ComfyUI (clone + venv + pip). This will take several minutes...");
            let mut log_lines: Vec<String> = Vec::new();
            let result = vibn_core::install_comfyui(|msg| log_lines.push(msg.to_owned()));
            for line in log_lines {
                state.append(line);
            }
            match result {
                Ok(()) => state.append("ComfyUI ready. Try /download-image-model comfyui:sdxl-base"),
                Err(error) => state.append(format!("Error: {error}")),
            }
            InputAction::Continue
        }
        "/start-comfy" => {
            match vibn_core::start_comfyui(&state.config) {
                Ok(()) => state.append("ComfyUI starting in background. See ~/.vibn/comfyui.log"),
                Err(error) => state.append(format!("Error: {error}")),
            }
            InputAction::Continue
        }
        "/stop-comfy" => {
            match vibn_core::stop_comfyui() {
                Ok(msg) => state.append(msg),
                Err(error) => state.append(format!("Error: {error}")),
            }
            InputAction::Continue
        }
        "/download-image-model" => {
            if arg.is_empty() {
                state.append("Usage: /download-image-model <model-key> (e.g. comfyui:sdxl-base)");
            } else {
                state.append(format!("Downloading checkpoint for {arg}..."));
                let mut log_lines: Vec<String> = Vec::new();
                let result =
                    vibn_core::download_checkpoint_for(arg, |msg| log_lines.push(msg.to_owned()));
                for line in log_lines {
                    state.append(line);
                }
                if let Err(error) = result {
                    state.append(format!("Error: {error}"));
                }
            }
            InputAction::Continue
        }
        "/tree" => {
            let depth = arg.parse::<u64>().ok().unwrap_or(3);
            let args = json!({"path": ".", "recursive": true, "depth": depth});
            let output = execute_tool(
                "list_directory",
                args.as_object().expect("object"),
                &state.config,
                &state.cwd,
            )
            .unwrap_or_else(|error| format!("Error: {error}"));
            state.append(output);
            InputAction::Continue
        }
        "/find" => {
            if arg.is_empty() {
                state.append("Usage: /find PATTERN");
            } else {
                let args = json!({"pattern": arg, "path": "."});
                let output = execute_tool(
                    "find_files",
                    args.as_object().expect("object"),
                    &state.config,
                    &state.cwd,
                )
                .unwrap_or_else(|error| format!("Error: {error}"));
                state.append(output);
            }
            InputAction::Continue
        }
        "/search" => {
            if arg.is_empty() {
                state.append("Usage: /search PATTERN");
            } else {
                let args = json!({"pattern": arg, "path": "."});
                let output = execute_tool(
                    "search_code",
                    args.as_object().expect("object"),
                    &state.config,
                    &state.cwd,
                )
                .unwrap_or_else(|error| format!("Error: {error}"));
                state.append(output);
            }
            InputAction::Continue
        }
        "/git" => {
            let args = json!({"args": if arg.is_empty() { "status" } else { arg }});
            let output = execute_tool(
                "git",
                args.as_object().expect("object"),
                &state.config,
                &state.cwd,
            )
            .unwrap_or_else(|error| format!("Error: {error}"));
            state.append(output);
            InputAction::Continue
        }
        "/remember" => {
            if arg.is_empty() {
                state.append("Usage: /remember TEXT");
            } else {
                let output = execute_tool(
                    "save_observation",
                    json!({"text": arg, "scope": "project"})
                        .as_object()
                        .expect("object"),
                    &state.config,
                    &state.cwd,
                )
                .unwrap_or_else(|error| format!("Error: {error}"));
                state.append(output);
            }
            InputAction::Continue
        }
        "/pin" => {
            if arg.is_empty() {
                state.append("Usage: /pin <note>  — pin a note that survives /compact");
            } else {
                append_pin(arg)?;
                state.append(format!("Pinned: {arg}"));
            }
            InputAction::Continue
        }
        "/clip" => {
            match last_agent_block(&state.output_lines) {
                Some(text) => match copy_to_clipboard(&text) {
                    Ok(()) => state.append(format!("Copied to clipboard ({}) chars", text.len())),
                    Err(error) => state.append(format!("Clipboard not available: {error}")),
                },
                None => state.append("Nothing to copy."),
            }
            InputAction::Continue
        }
        "/open" => {
            if arg.is_empty() {
                state.append("Usage: /open <file>");
            } else {
                let path = resolve_cd_path(&state.cwd, arg);
                match open_in_editor(&path) {
                    Ok(()) => state.append(format!("Opened {}", path.display())),
                    Err(error) => state.append(format!("Error: {error}")),
                }
            }
            InputAction::Continue
        }
        "/pins" => {
            let pins = load_pins()?;
            if pins.is_empty() {
                state.append("No pinned notes. Use /pin <note> to add one.");
            } else {
                let mut lines = vec!["Pinned notes:".to_owned()];
                lines.extend(
                    pins.into_iter()
                        .enumerate()
                        .map(|(index, pin)| format!("  {}. {}", index + 1, pin)),
                );
                state.append(lines.join("\n"));
            }
            InputAction::Continue
        }
        "/memory" => {
            match project_memory_entries(&state.cwd) {
                Ok(entries) if entries.is_empty() => {
                    state.append("No memories for this project. Use /remember <fact>")
                }
                Ok(entries) => {
                    let mut lines = vec![format!("Project memories ({})", state.cwd.display())];
                    lines.extend(
                        entries
                            .iter()
                            .enumerate()
                            .map(|(index, entry)| format!("  {}. {}", index + 1, entry.text)),
                    );
                    lines.push(String::new());
                    lines.push("/forget <n> to remove".to_owned());
                    state.append(lines.join("\n"));
                }
                Err(error) => state.append(format!("Error: {error}")),
            }
            InputAction::Continue
        }
        "/forget" => {
            match arg.parse::<usize>() {
                Ok(index) => match forget_project_memory(&state.cwd, index) {
                    Ok(Some(entry)) => state.append(format!("Forgotten: {}", entry.text)),
                    Ok(None) => state.append(format!("No memory #{index}")),
                    Err(error) => state.append(format!("Error: {error}")),
                },
                Err(_) => state.append("Usage: /forget <number>"),
            }
            InputAction::Continue
        }
        "/transcripts" | "/sessions" => {
            let entries = list_transcripts(20)?;
            if entries.is_empty() {
                state.append("No transcripts saved yet.");
            } else {
                let lines = entries
                    .into_iter()
                    .map(|entry| {
                        format!(
                            "- {}  {}  {} messages  {}",
                            entry.session_id, entry.timestamp, entry.messages, entry.project
                        )
                    })
                    .collect::<Vec<_>>();
                state.append(lines.join("\n"));
            }
            InputAction::Continue
        }
        "/prompts" => {
            let prompts = load_prompts()?;
            if prompts.is_empty() {
                state.append("No saved prompts. Use /prompt-save <name> <text> to save one.");
            } else {
                let mut lines = vec!["Saved prompts:".to_owned()];
                lines.extend(prompts.into_iter().map(|(name, text)| {
                    let preview = if text.chars().count() > 80 {
                        format!("{}…", text.chars().take(80).collect::<String>())
                    } else {
                        text
                    };
                    format!("  {name}  {preview}")
                }));
                state.append(lines.join("\n"));
            }
            InputAction::Continue
        }
        "/prompt-save" => {
            let mut parts = arg.splitn(2, char::is_whitespace);
            let name = parts.next().unwrap_or("").trim();
            let text = parts.next().unwrap_or("").trim();
            if name.is_empty() || text.is_empty() {
                state.append("Usage: /prompt-save <name> <prompt text>");
            } else {
                let mut prompts = load_prompts()?;
                prompts.insert(name.to_owned(), text.to_owned());
                save_prompts(&prompts)?;
                state.append(format!("Saved prompt {name}"));
            }
            InputAction::Continue
        }
        "/prompt-run" => {
            if arg.is_empty() {
                state.append("Usage: /prompt-run <name>");
            } else {
                let prompts = load_prompts()?;
                let name = arg.trim();
                if let Some(text) = prompts.get(name) {
                    set_input_text(input, text);
                    state.append(format!(
                        "Loaded prompt '{name}' into input — press Enter to run"
                    ));
                } else {
                    state.append(format!("No prompt named '{name}'. Use /prompts to list."));
                }
            }
            InputAction::Continue
        }
        "/watch" => {
            if let Some((path_arg, prompt_template)) = parse_watch_command(arg) {
                let watch_path = resolve_cd_path(&state.cwd, &path_arg);
                let watch_key = watch_path.display().to_string();
                if let Some(existing) = state.watchers.remove(&watch_key) {
                    existing.active.store(false, Ordering::Relaxed);
                }
                let active = Arc::new(AtomicBool::new(true));
                spawn_watch_task(
                    watch_path.clone(),
                    state.cwd.clone(),
                    prompt_template.clone(),
                    active.clone(),
                    tx.clone(),
                );
                state
                    .watchers
                    .insert(watch_key.clone(), WatchHandle { active });
                state.append(format!(
                    "Watching {}\nPrompt: {}\nUse /unwatch to stop.",
                    watch_key,
                    prompt_template.chars().take(80).collect::<String>()
                ));
            } else {
                state.append("Usage: /watch <path> [prompt]  — trigger agent when files change");
            }
            InputAction::Continue
        }
        "/unwatch" => {
            let stopped = state.stop_all_watchers();
            if stopped == 0 {
                state.append("No active watchers.");
            } else {
                state.append(format!("Stopped {stopped} watcher(s)"));
            }
            InputAction::Continue
        }
        "/bg" => {
            if arg.is_empty() {
                state.append("Usage: /bg <prompt>  — run agent task in background");
            } else {
                let id = state.next_background_task_id;
                state.next_background_task_id += 1;
                state.background_tasks.push(BackgroundTask {
                    id,
                    prompt: arg.to_owned(),
                    status: "running",
                    result_summary: String::new(),
                });
                state.append(format!(
                    "Background #{id}: {}",
                    arg.chars().take(60).collect::<String>()
                ));
                spawn_background_task(
                    id,
                    arg.to_owned(),
                    state.model.clone(),
                    state.plan_mode,
                    state.project_context.clone(),
                    state.cwd.clone(),
                    state.config.command_timeout_secs(),
                    tx.clone(),
                );
            }
            InputAction::Continue
        }
        "/tasks" => {
            if state.background_tasks.is_empty() {
                state.append("No background tasks.");
            } else {
                let lines = state
                    .background_tasks
                    .iter()
                    .map(|task| {
                        format!(
                            "  #{} {}  {}",
                            task.id,
                            task.status,
                            task.prompt.chars().take(60).collect::<String>()
                        )
                    })
                    .collect::<Vec<_>>();
                state.append(lines.join("\n"));
            }
            InputAction::Continue
        }
        "/export-training-data" => {
            if state.processing {
                state.append("Still processing the previous request.");
            } else {
                match parse_transcript_export_args(arg, &state.cwd).and_then(export_training_data) {
                    Ok(message) => state.append(message),
                    Err(error) => state.append(format!("Error: {error}")),
                }
            }
            InputAction::Continue
        }
        "/generate-training-data" => {
            if state.processing {
                state.append("Still processing the previous request.");
            } else {
                match parse_training_generate_args(arg, &state.cwd) {
                    Ok(args) => {
                        state.append("Running generator... (this will take a while)");
                        state.processing = true;
                        spawn_training_generate_task(args, state.cwd.clone(), tx.clone());
                    }
                    Err(error) => {
                        state.append(format!("Error: {error}"));
                    }
                }
            }
            InputAction::Continue
        }
        "/test" => {
            if !arg.is_empty() {
                state.test_cmd = arg.to_owned();
            }
            if state.test_cmd.is_empty() {
                state.append(
                    "Usage: /test <command>   e.g. /test cargo test\nOnce set, /test re-runs the last command.",
                );
            } else if state.processing {
                state.append("Still processing the previous request.");
            } else {
                state.processing = true;
                spawn_test_loop_task(
                    state.test_cmd.clone(),
                    state.model.clone(),
                    state.project_context.clone(),
                    state.session_id.clone(),
                    state.cwd.clone(),
                    state.config.clone(),
                    tx.clone(),
                );
            }
            InputAction::Continue
        }
        "/compare" => {
            if state.processing {
                state.append("Still processing the previous request.");
            } else if let Some((model_b, prompt)) = parse_compare_command(arg, &state.model) {
                state.processing = true;
                spawn_compare_task(
                    state.model.clone(),
                    model_b,
                    prompt,
                    state.plan_mode,
                    state.project_context.clone(),
                    state.cwd.clone(),
                    state.config.command_timeout_secs(),
                    tx,
                );
            } else {
                state.append("Usage: /compare <prompt>  — run prompt on two models and compare");
            }
            InputAction::Continue
        }
        "/diff" => {
            let command = if arg.is_empty() {
                "git diff".to_owned()
            } else {
                format!("git diff {arg}")
            };
            let output = execute_tool(
                "run_command",
                json!({"command": command}).as_object().expect("object"),
                &state.config,
                &state.cwd,
            )
            .unwrap_or_else(|error| format!("Error: {error}"));
            if output.trim() == "[no output]" {
                state.append("No changes.");
            } else {
                state.append(output);
            }
            InputAction::Continue
        }
        "/compact" => {
            let (_, messages) = load_transcript(&state.session_id)?;
            let client =
                OllamaClient::new(Duration::from_secs(state.config.command_timeout_secs()))?;
            let (compacted, message) =
                compact_session_messages(&client, &state.model, messages, &state.config);
            if let Some(compacted) = compacted {
                save_session_messages(state, &compacted)?;
                state.output_lines = render_messages(&compacted);
                state.scroll_offset = 0;
                state.auto_scroll = true;
            }
            state.append(message);
            InputAction::Continue
        }
        "/tokens" => {
            let (_, messages) = load_transcript(&state.session_id)?;
            let usage = token_usage(&state.model, &messages);
            let filled = ((30.0 * (usage.percent / 100.0)).floor() as usize).min(30);
            let bar = format!("{}{}", "█".repeat(filled), "░".repeat(30 - filled));
            let mut lines = vec![format!(
                "[{bar}] {} / {} runtime tokens ({}%)",
                usage.used, usage.limit, usage.percent
            )];
            lines.push(format!(
                "Model context: {}  |  Runtime ctx: {}",
                usage.context_window, usage.runtime_context_window
            ));
            lines.push(format!(
                "Messages: {}  |  Remaining: ~{} tokens",
                messages.len(),
                usage.remaining
            ));
            if usage.needs_warning {
                lines.push(
                    "Warning: approaching context limit. Use /compact to free space.".to_owned(),
                );
            }
            state.append(lines.join("\n"));
            InputAction::Continue
        }
        "/commit" => {
            let staged = run_git_capture(&state.cwd, &["diff", "--staged", "--stat"])?;
            let mut summary = command_output_text(&staged);
            if summary.is_empty() {
                let unstaged = run_git_capture(&state.cwd, &["diff", "--stat"])?;
                summary = command_output_text(&unstaged);
                if summary.is_empty() {
                    state.append("Nothing to commit.");
                    return Ok(InputAction::Continue);
                }
                state.append("No staged changes — staging all modified files.");
                let added = run_git_capture(&state.cwd, &["add", "-A"])?;
                if !added.status.success() {
                    state.append(command_output_text(&added));
                    return Ok(InputAction::Continue);
                }
            }
            if !summary.is_empty() {
                state.append(summary);
            }
            let diff = run_git_capture(&state.cwd, &["diff", "--staged"])?;
            let diff_text = String::from_utf8_lossy(&diff.stdout).to_string();
            if diff_text.trim().is_empty() {
                state.append("Nothing to commit.");
                return Ok(InputAction::Continue);
            }

            state.append("Generating commit message…");
            let (_, prior_messages) = load_transcript(&state.session_id)?;
            let client =
                OllamaClient::new(Duration::from_secs(state.config.command_timeout_secs()))?;
            let prompt = format!(
                "Write a concise git commit message for these changes. Format: one subject line (≤72 chars, imperative mood), blank line, then bullet points for details if needed. Output ONLY the commit message, no explanation.\n\n```diff\n{}\n```",
                &diff_text.chars().take(6000).collect::<String>()
            );
            let generated = client.chat(
                &state.model,
                build_request_messages(
                    prior_messages,
                    &prompt,
                    false,
                    &state.project_context,
                    &state.cwd,
                ),
            )?;
            let mut message = normalize_commit_message(&generated);
            if message.is_empty() {
                state.append("Error: generated empty commit message");
                return Ok(InputAction::Continue);
            }

            let choice = prompt_commit_confirmation(&message)?;
            if choice == "n" {
                state.append("Commit cancelled.");
                return Ok(InputAction::Continue);
            }
            if choice == "edit" {
                let temp_path = temp_commit_message_path();
                fs::write(&temp_path, &message)?;
                open_in_editor(&temp_path)?;
                message = normalize_commit_message(&fs::read_to_string(&temp_path)?);
                let _ = fs::remove_file(&temp_path);
                if message.is_empty() {
                    state.append("Commit cancelled.");
                    return Ok(InputAction::Continue);
                }
            }

            let commit = Command::new("git")
                .args(["commit", "-m", &message])
                .current_dir(&state.cwd)
                .output()?;
            state.append(command_output_text(&commit));
            InputAction::Continue
        }
        "/allow" | "/deny" => {
            match apply_permission_rule(&mut state.config, command, arg) {
                Ok(message) => state.append(message),
                Err(error) => state.append(format!("Error: {error}")),
            }
            InputAction::Continue
        }
        "/config" => {
            if arg.is_empty() {
                state.append(render_config(&state.config)?);
            } else {
                match apply_config_assignment(&mut state.config, arg) {
                    Ok(message) => state.append(message),
                    Err(error) => state.append(format!("Error: {error}")),
                }
            }
            InputAction::Continue
        }
        "/plan" => {
            state.plan_mode = !state.plan_mode;
            if state.plan_mode {
                state.append("Plan mode ON — agent will outline steps before acting.");
            } else {
                state.append("Plan mode off.");
            }
            InputAction::Continue
        }
        _ => {
            state.append(format!("Unknown command: {command_text}. Type /help to see commands."));
            InputAction::Continue
        }
    };

    if let Err(error) = save_config(&state.config) {
        state.append(format!("Error: failed to save config: {error}"));
    }

    Ok(action)
}

fn set_config_string_field(
    state: &mut UiState,
    key: &str,
    value: &str,
    label: &str,
) -> InputAction {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        let current = state
            .config
            .extra
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("(unset)")
            .to_owned();
        state.append(format!("Current {label}: {current}"));
    } else {
        state
            .config
            .extra
            .insert(key.to_owned(), serde_json::Value::String(trimmed.to_owned()));
        state.append(format!("{label} set to {trimmed}"));
    }
    InputAction::Continue
}

fn spawn_agent_task(
    prompt: String,
    plan_mode: bool,
    project_context: String,
    model: String,
    session_id: String,
    cwd: PathBuf,
    config: AppConfig,
    tx: Sender<UiEvent>,
) {
    thread::spawn(move || {
        let _ = tx.send(UiEvent::SetProcessing(true));
        let result = (|| -> Result<String, String> {
            let (_, prior_messages) =
                load_transcript(&session_id).map_err(|error| error.to_string())?;
            let client = OllamaClient::new(Duration::from_secs(config.command_timeout_secs()))
                .map_err(|error| error.to_string())?;
            let mut confirm = |tool: &str, args: &Map<String, Value>, reason: &str| -> bool {
                let (reply_tx, reply_rx) = std::sync::mpsc::channel();
                let _ = tx.send(UiEvent::RequestConfirm {
                    tool: tool.to_owned(),
                    cmd: summarize_tool_args(tool, args),
                    reason: reason.to_owned(),
                    reply: reply_tx,
                });
                let decision = reply_rx
                    .recv_timeout(Duration::from_secs(30))
                    .unwrap_or(false);
                let _ = tx.send(UiEvent::ClearConfirm);
                decision
            };
            let mut diff = |path: &Path, old_content: &str, new_content: &str| -> bool {
                let (reply_tx, reply_rx) = std::sync::mpsc::channel();
                let _ = tx.send(UiEvent::RequestDiff {
                    path: path.display().to_string(),
                    diff_lines: render_diff_lines(path, old_content, new_content),
                    reply: reply_tx,
                });
                let decision = reply_rx
                    .recv_timeout(Duration::from_secs(120))
                    .unwrap_or(true);
                let _ = tx.send(UiEvent::ClearDiff);
                decision
            };
            let mut confirm_callback = Some(&mut confirm);
            let mut diff_callback = Some(&mut diff);
            let result = run_agent_turns_with_callbacks(
                &client,
                &model,
                build_request_messages(prior_messages, &prompt, plan_mode, &project_context, &cwd),
                &config,
                &cwd,
                &mut confirm_callback,
                &mut diff_callback,
            )
            .map_err(|error| error.to_string())?;
            save_transcript(
                &session_id,
                &result.messages,
                Some(agent_metadata(&model, &cwd)),
            )
            .map_err(|error| error.to_string())?;
            let mut parts = Vec::new();
            if let Some(summary) = result.auto_compact_summary {
                parts.push(format!("⟳ Auto-compacted context. {summary}"));
            }
            parts.push(result.final_text);
            Ok(parts.join("\n"))
        })();
        let _ = tx.send(UiEvent::Append(
            result.unwrap_or_else(|error| format!("Error: {error}")),
        ));
        let _ = tx.send(UiEvent::SetProcessing(false));
    });
}

fn spawn_shell_task(command: String, cwd: PathBuf, config: AppConfig, tx: Sender<UiEvent>) {
    thread::spawn(move || {
        let _ = tx.send(UiEvent::SetProcessing(true));
        let args = json!({ "command": command });
        let result = execute_tool(
            "run_command",
            args.as_object().expect("object"),
            &config,
            &cwd,
        )
        .unwrap_or_else(|error| format!("Error: {error}"));
        let _ = tx.send(UiEvent::Append(result));
        let _ = tx.send(UiEvent::Append(String::new()));
        let _ = tx.send(UiEvent::SetProcessing(false));
    });
}

fn spawn_compare_task(
    model_a: String,
    model_b: String,
    prompt: String,
    plan_mode: bool,
    project_context: String,
    cwd: PathBuf,
    timeout_secs: u64,
    tx: Sender<UiEvent>,
) {
    thread::spawn(move || {
        let _ = tx.send(UiEvent::SetProcessing(true));
        let divider = "─".repeat(size().map(|(width, _)| width as usize).unwrap_or(80));
        let _ = tx.send(UiEvent::Append(format!(
            "Compare: {model_a} vs {model_b}\nPrompt: {prompt}\n"
        )));
        let run = |model: &str| -> String {
            let client = match OllamaClient::new(Duration::from_secs(timeout_secs)) {
                Ok(client) => client,
                Err(error) => return format!("Error: {error}"),
            };
            let messages =
                build_request_messages(Vec::new(), &prompt, plan_mode, &project_context, &cwd);
            match client.chat(model, messages) {
                Ok(text) => text,
                Err(error) => format!("Error: {error}"),
            }
        };
        let result_a = run(&model_a);
        let result_b = run(&model_b);
        let _ = tx.send(UiEvent::Append(format!("── {model_a} ──")));
        let _ = tx.send(UiEvent::Append(result_a));
        let _ = tx.send(UiEvent::Append(format!("\n── {model_b} ──")));
        let _ = tx.send(UiEvent::Append(result_b));
        let _ = tx.send(UiEvent::Append(divider));
        let _ = tx.send(UiEvent::SetProcessing(false));
    });
}

fn spawn_watch_task(
    watch_path: PathBuf,
    cwd: PathBuf,
    prompt_template: String,
    active: Arc<AtomicBool>,
    tx: Sender<UiEvent>,
) {
    thread::spawn(move || {
        let mut previous = collect_watch_snapshot(&watch_path);
        while active.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(1500));
            if !active.load(Ordering::Relaxed) {
                break;
            }
            let current = collect_watch_snapshot(&watch_path);
            let changed = current
                .iter()
                .filter_map(|(path, modified)| match previous.get(path) {
                    Some(prev) if prev == modified => None,
                    _ => Some(path.clone()),
                })
                .collect::<Vec<_>>();
            previous = current;
            if changed.is_empty() {
                continue;
            }
            let relative = changed
                .iter()
                .map(|path| {
                    path.strip_prefix(&cwd)
                        .map(|relative| relative.display().to_string())
                        .unwrap_or_else(|_| path.display().to_string())
                })
                .collect::<Vec<_>>();
            let changed_preview = relative
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            let prompt = prompt_template.replace(
                "{path}",
                &relative
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            let _ = tx.send(UiEvent::WatchTriggered {
                prompt,
                changed: changed_preview,
            });
        }
    });
}

fn spawn_background_task(
    id: usize,
    prompt: String,
    model: String,
    plan_mode: bool,
    project_context: String,
    cwd: PathBuf,
    timeout_secs: u64,
    tx: Sender<UiEvent>,
) {
    thread::spawn(move || {
        let client = match OllamaClient::new(Duration::from_secs(timeout_secs)) {
            Ok(client) => client,
            Err(error) => {
                let _ = tx.send(UiEvent::BackgroundFinished {
                    id,
                    status: "error",
                    summary: error.to_string(),
                });
                return;
            }
        };
        let messages =
            build_request_messages(Vec::new(), &prompt, plan_mode, &project_context, &cwd);
        match client.chat(&model, messages) {
            Ok(text) => {
                let _ = tx.send(UiEvent::BackgroundFinished {
                    id,
                    status: "done",
                    summary: text.chars().take(300).collect(),
                });
            }
            Err(error) => {
                let _ = tx.send(UiEvent::BackgroundFinished {
                    id,
                    status: "error",
                    summary: error.to_string(),
                });
            }
        }
    });
}

fn spawn_test_loop_task(
    command: String,
    model: String,
    project_context: String,
    session_id: String,
    cwd: PathBuf,
    config: AppConfig,
    tx: Sender<UiEvent>,
) {
    thread::spawn(move || {
        const MAX_ATTEMPTS: usize = 5;
        let _ = tx.send(UiEvent::SetProcessing(true));
        let _ = tx.send(UiEvent::Append(format!("Running: {command}")));

        for attempt in 1..=MAX_ATTEMPTS {
            let output = execute_tool(
                "run_command",
                json!({ "command": command }).as_object().expect("object"),
                &config,
                &cwd,
            )
            .unwrap_or_else(|error| format!("Error: {error}"));
            let _ = tx.send(UiEvent::Append(output.clone()));

            let passed = !output.contains("[exit code:") && !output.contains("[timed out after");
            if passed {
                let _ = tx.send(UiEvent::Append(format!(
                    "✓ Tests passed (attempt {attempt})"
                )));
                let _ = tx.send(UiEvent::SetProcessing(false));
                return;
            }

            if attempt == MAX_ATTEMPTS {
                let _ = tx.send(UiEvent::Append(format!(
                    "Tests still failing after {MAX_ATTEMPTS} attempts."
                )));
                let _ = tx.send(UiEvent::SetProcessing(false));
                return;
            }

            let failure_prompt = format!(
                "The test command `{command}` failed (attempt {attempt}/{MAX_ATTEMPTS}).\nOutput:\n```\n{}\n```\nFix the failing tests. Do not change the tests themselves unless they are clearly wrong.",
                output
                    .chars()
                    .rev()
                    .take(3000)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>()
            );
            let _ = tx.send(UiEvent::Append(format!(
                "Attempt {attempt} failed — asking agent to fix…"
            )));

            let result = (|| -> Result<String, String> {
                let (_, prior_messages) =
                    load_transcript(&session_id).map_err(|error| error.to_string())?;
                let client = OllamaClient::new(Duration::from_secs(config.command_timeout_secs()))
                    .map_err(|error| error.to_string())?;
                let mut no_confirm = |_: &str, _: &Map<String, Value>, _: &str| true;
                let mut no_diff = |_: &Path, _: &str, _: &str| true;
                let mut confirm_callback = Some(&mut no_confirm);
                let mut diff_callback = Some(&mut no_diff);
                let result = run_agent_turns_with_callbacks(
                    &client,
                    &model,
                    build_request_messages(
                        prior_messages,
                        &failure_prompt,
                        false,
                        &project_context,
                        &cwd,
                    ),
                    &config,
                    &cwd,
                    &mut confirm_callback,
                    &mut diff_callback,
                )
                .map_err(|error| error.to_string())?;
                save_transcript(
                    &session_id,
                    &result.messages,
                    Some(agent_metadata(&model, &cwd)),
                )
                .map_err(|error| error.to_string())?;
                let mut parts = Vec::new();
                if let Some(summary) = result.auto_compact_summary {
                    parts.push(format!("⟳ Auto-compacted context. {summary}"));
                }
                parts.push(result.final_text);
                Ok(parts.join("\n"))
            })();
            let _ = tx.send(UiEvent::Append(
                result.unwrap_or_else(|error| format!("Error: {error}")),
            ));
        }

        let _ = tx.send(UiEvent::SetProcessing(false));
    });
}

fn build_request_messages(
    prior_messages: Vec<ChatMessage>,
    user_prompt: &str,
    plan_mode: bool,
    project_context: &str,
    cwd: &Path,
) -> Vec<ChatMessage> {
    let mut system_prompt =
        "You are Vibn — a local AI coding agent. Respond directly, clearly, and helpfully."
            .to_owned();
    if plan_mode {
        system_prompt.push_str("\n\n## PLAN MODE\nBefore calling ANY tools, write a numbered step-by-step plan of what you'll do and which files/commands you'll touch. Present it clearly, then wait for the user to say 'go' or 'proceed'. Do not call any tools until explicitly told to.");
    }
    if !project_context.trim().is_empty() {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(project_context.trim());
    }
    let mcp_summary = connected_mcp_summary();
    if !mcp_summary.is_empty() {
        system_prompt.push_str("\n\n## Connected MCP Servers\n");
        system_prompt.push_str(&mcp_summary);
    }
    if let Ok(pins) = load_pins() {
        if let Some(block) = pinned_notes_block(&pins) {
            system_prompt.push_str("\n\n");
            system_prompt.push_str(&block);
        }
    }
    if let Ok(Some(block)) = remembered_facts_block(cwd) {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(&block);
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

fn pinned_notes_block(pins: &[String]) -> Option<String> {
    if pins.is_empty() {
        return None;
    }
    let mut lines = vec!["## Pinned Notes (always remember)".to_owned()];
    lines.extend(pins.iter().map(|pin| format!("- {pin}")));
    Some(lines.join("\n"))
}

fn expand_file_mentions(text: &str, cwd: &Path) -> String {
    let bytes = text.as_bytes();
    let mut result = String::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'@' {
            let start = index + 1;
            let mut end = start;
            while end < bytes.len() {
                let ch = bytes[end] as char;
                if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '/' | '_' | '-') {
                    end += 1;
                } else {
                    break;
                }
            }
            if end > start {
                let filepath = &text[start..end];
                let full = resolve_cd_path(cwd, filepath);
                if full.is_file() {
                    if let Ok(content) = fs::read_to_string(&full) {
                        let ext = Path::new(filepath)
                            .extension()
                            .and_then(|value| value.to_str())
                            .unwrap_or("");
                        result.push_str(&format!("\n\n`{filepath}`:\n```{ext}\n{content}\n```"));
                        index = end;
                        continue;
                    }
                }
            }
        }
        result.push(bytes[index] as char);
        index += 1;
    }
    result
}

fn agent_metadata(model: &str, cwd: &Path) -> Map<String, Value> {
    let mut metadata = Map::new();
    metadata.insert("model".into(), Value::String(model.to_owned()));
    metadata.insert("mode".into(), Value::String("agent".into()));
    metadata.insert("project".into(), Value::String(cwd.display().to_string()));
    metadata
}

fn save_session_messages(
    state: &UiState,
    messages: &[ChatMessage],
) -> Result<(), Box<dyn std::error::Error>> {
    save_transcript(
        &state.session_id,
        messages,
        Some(agent_metadata(&state.model, &state.cwd)),
    )
}

fn truncate_last_turn(messages: &[ChatMessage]) -> Option<Vec<ChatMessage>> {
    let mut cutoff = messages.len();
    while cutoff > 0 && messages[cutoff - 1].role != "user" {
        cutoff -= 1;
    }
    if cutoff == 0 {
        None
    } else {
        Some(messages[..cutoff - 1].to_vec())
    }
}

fn compact_session_messages(
    client: &OllamaClient,
    model: &str,
    messages: Vec<ChatMessage>,
    config: &AppConfig,
) -> (Option<Vec<ChatMessage>>, String) {
    if messages.len() <= 5 {
        return (None, "Nothing to compact.".to_owned());
    }

    let recent_count = 6usize;
    let split_at = messages.len().saturating_sub(recent_count);
    let old = messages[..split_at].to_vec();
    let recent = messages[split_at..].to_vec();
    if old.is_empty() {
        return (None, "Nothing to compact.".to_owned());
    }

    run_hooks(config, HOOK_PRE_COMPACT, None);
    let summary = summarize_compacted_messages(client, model, &old)
        .unwrap_or_else(|| heuristic_summary(&old));
    let mut compacted = vec![
        ChatMessage::user(summary),
        ChatMessage::assistant("Got it."),
    ];
    compacted.extend(recent);
    run_hooks(config, HOOK_POST_COMPACT, None);
    (
        Some(compacted),
        format!("Compacted {} messages.", old.len()),
    )
}

fn summarize_compacted_messages(
    client: &OllamaClient,
    model: &str,
    old: &[ChatMessage],
) -> Option<String> {
    let mut lines = Vec::new();
    for msg in old {
        let content = msg.content.trim();
        match msg.role.as_str() {
            "user" => lines.push(format!("User: {}", truncate_for_summary(content, 400))),
            "assistant" => {
                if !msg.tool_calls.is_empty() {
                    let names = msg
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
                truncate_for_summary(content.lines().next().unwrap_or_default(), 120)
            )),
            _ => {}
        }
    }
    let conv = lines.into_iter().rev().take(40).collect::<Vec<_>>();
    let conv = conv.into_iter().rev().collect::<Vec<_>>().join("\n");
    let prompt = format!(
        "Summarize this coding agent session in 5-8 bullet points. Focus on: what was built or changed, which files were modified, key decisions, and current state. Be specific and factual.\n\n{conv}\n\nSummary:"
    );
    let response = client.chat(model, vec![ChatMessage::user(prompt)]).ok()?;
    let summary = response.trim();
    if summary.is_empty() {
        None
    } else {
        Some(format!("[Compacted session summary]\n{summary}"))
    }
}

fn heuristic_summary(old: &[ChatMessage]) -> String {
    let mut parts = Vec::new();
    let mut index = 0usize;
    while index < old.len() {
        let msg = &old[index];
        let content = msg.content.trim();
        match msg.role.as_str() {
            "user" => parts.push(format!("User: {}", truncate_for_summary(content, 150))),
            "assistant" => {
                if !msg.tool_calls.is_empty() {
                    let names = msg
                        .tool_calls
                        .iter()
                        .map(|call| call.function.name.as_str())
                        .collect::<Vec<_>>();
                    let mut results = Vec::new();
                    let mut next = index + 1;
                    while next < old.len() && old[next].role == "tool" {
                        results.push(truncate_for_summary(
                            old[next].content.lines().next().unwrap_or_default(),
                            80,
                        ));
                        next += 1;
                    }
                    parts.push(format!(
                        "Agent used {} → {}",
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

fn truncate_for_summary(text: &str, max_chars: usize) -> String {
    let mut result = String::new();
    for ch in text.chars().take(max_chars) {
        result.push(ch);
    }
    result
}

#[cfg(test)]
fn render_model_storage_summary(config: &AppConfig) -> String {
    let storage_path = get_ollama_models_path(config).unwrap_or_else(|| PathBuf::from("~/models"));
    let profile = build_system_profile(storage_path.clone());
    let disk = profile
        .storage_free_gb
        .map(|free| format!("{free:.1}GB free"))
        .unwrap_or_else(|| "disk unknown".to_owned());
    format!(
        "Model storage path: {}\nSystem: {}\nStorage: {}",
        storage_path.display(),
        vibn_core::format_system_summary(&profile),
        disk
    )
}

fn render_config(config: &AppConfig) -> Result<String, Box<dyn std::error::Error>> {
    Ok(serde_json::to_string_pretty(config)?)
}

fn apply_config_assignment(
    config: &mut AppConfig,
    assignment: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let (key, raw_value) = assignment
        .split_once('=')
        .ok_or_else(|| "Usage: /config key=value".to_owned())?;
    let key = key.trim();
    if key.is_empty() {
        return Err("Usage: /config key=value".into());
    }
    let parsed_value = serde_json::from_str::<Value>(raw_value.trim())
        .unwrap_or(Value::String(raw_value.trim().to_owned()));
    let mut value = serde_json::to_value(config.clone())?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "Config is not an object".to_owned())?;
    object.insert(key.to_owned(), parsed_value.clone());
    *config = serde_json::from_value(value)?;
    Ok(format!("{key} = {parsed_value}"))
}

fn apply_model_path_change(
    config: &mut AppConfig,
    cwd: &Path,
    raw_path: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let resolved = resolve_cd_path(cwd, raw_path);
    fs::create_dir_all(&resolved)?;
    let canonical = resolved.canonicalize().unwrap_or(resolved);
    config.ollama_models_path = canonical.display().to_string();
    Ok(format!(
        "Model storage path set to {}",
        config.ollama_models_path
    ))
}

fn apply_permission_rule(
    config: &mut AppConfig,
    command: &str,
    arg: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    if arg.trim().is_empty() {
        return Err(format!("Usage: {command} <tool> [pattern]").into());
    }

    let mut parts = arg.splitn(2, char::is_whitespace);
    let tool_name = parts.next().unwrap_or("").trim();
    let pattern = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if tool_name.is_empty() {
        return Err(format!("Usage: {command} <tool> [pattern]").into());
    }

    let mut rule = Map::new();
    rule.insert("tool".to_owned(), Value::String(tool_name.to_owned()));
    if let Some(pattern_text) = pattern {
        let key = if matches!(
            tool_name,
            "write_file" | "read_file" | "edit_file" | "patch_file"
        ) {
            "paths"
        } else {
            "commands"
        };
        rule.insert(
            key.to_owned(),
            Value::Array(vec![Value::String(pattern_text.to_owned())]),
        );
    }

    let permissions = config
        .extra
        .entry("permissions".to_owned())
        .or_insert_with(|| json!({"always_allow": [], "always_deny": [], "confirm": []}));
    let permissions = permissions
        .as_object_mut()
        .ok_or_else(|| "permissions config is not an object".to_owned())?;
    let bucket_name = if command == "/allow" {
        "always_allow"
    } else {
        "always_deny"
    };
    let bucket = permissions
        .entry(bucket_name.to_owned())
        .or_insert_with(|| Value::Array(Vec::new()));
    let bucket = bucket
        .as_array_mut()
        .ok_or_else(|| format!("{bucket_name} permissions bucket is not an array"))?;

    let rule_value = Value::Object(rule);
    if bucket.contains(&rule_value) {
        return Ok("Rule already exists.".to_owned());
    }
    bucket.push(rule_value);

    let description = if let Some(pattern_text) = pattern {
        format!("{tool_name} ({pattern_text})")
    } else {
        tool_name.to_owned()
    };
    Ok(format!("{bucket_name}: {description}"))
}

fn render(frame: &mut Frame<'_>, state: &UiState, input: &TextArea<'_>) {
    let input_height = input.lines().len().clamp(1, 6) as u16;
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(input_height),
        ])
        .split(frame.area());

    let header = Line::from(vec![
        Span::styled(" Vibn ", Style::default().fg(Color::Black).bg(Color::Green)),
        Span::raw(format!(
            "  model:{}  cwd:{}",
            state.model,
            state.cwd.display()
        )),
    ]);
    frame.render_widget(Paragraph::new(header), layout[0]);
    frame.render_widget(separator(frame.area().width), layout[1]);

    let output_body = if let Some(pending_diff) = state.pending_diff.as_ref() {
        pending_diff.diff_lines.join("\n")
    } else if state.review_mode.active {
        render_review_lines(&state.review_mode).join("\n")
    } else if let Some(browser) = state.browser_mode.as_ref() {
        render_browser_lines(browser, layout[2].width).join("\n")
    } else if state.install_prompt.is_some() {
        state.output_lines.join("\n")
    } else if state.constraints_mode.active {
        render_constraints_lines(&state.constraints_mode).join("\n")
    } else {
        state.output_lines.join("\n")
    };
    let output = if output_body.is_empty() {
        Paragraph::new("")
    } else {
        Paragraph::new(output_body).wrap(Wrap { trim: false })
    };
    frame.render_widget(output.scroll((state.scroll_offset, 0)), layout[2]);

    if !state.constraints_mode.active
        && !state.review_mode.active
        && state.browser_mode.is_none()
        && state.install_prompt.is_none()
    {
        if let Some(context) = slash_context(&current_input_text(input), &state.config, &state.cwd)
        {
            if !context.items.is_empty() && context.exact_description.is_none() {
                render_completion_popup(frame, layout[2], &context, state.completion_index);
            }
        }
    }

    let status = if let Some(pending_diff) = state.pending_diff.as_ref() {
        Line::from(vec![Span::styled(
            format!(
                " DIFF  {}   [y] apply   [n] skip ",
                Path::new(&pending_diff.path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or(&pending_diff.path)
            ),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )])
    } else if let Some(pending_confirm) = state.pending_confirm.as_ref() {
        let mut cmd = pending_confirm.cmd.clone();
        if cmd.len() > 80 {
            cmd.truncate(79);
            cmd.push('…');
        }
        let mut reason = pending_confirm.reason.clone();
        if reason.len() > 48 {
            reason.truncate(47);
            reason.push('…');
        }
        Line::from(vec![Span::styled(
            format!(
                " ⚠ {}: {}  ({})  [Y]es / [N]o / Esc=deny ",
                pending_confirm.tool, cmd, reason
            ),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )])
    } else if state.review_mode.active {
        Line::from(vec![Span::styled(
            " REVIEW  y approve   n discard   s skip   e edit   q quit ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )])
    } else if let Some(install) = state.install_prompt.as_ref() {
        let prompt = current_install_prompt(install).unwrap_or_else(|| "value".to_owned());
        Line::from(vec![Span::styled(
            format!(
                " INSTALL {}  {}  Enter save   Esc cancel ",
                install.server_name, prompt
            ),
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )])
    } else if let Some(browser) = state.browser_mode.as_ref() {
        let filter_note = if browser.filter.trim().is_empty() {
            String::new()
        } else {
            format!("  filter: {}", browser.filter)
        };
        Line::from(vec![Span::styled(
            format!(" MENU {}{}", browser.footer, filter_note),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )])
    } else if state.constraints_mode.active {
        let label = if state.constraints_mode.adding || state.constraints_mode.editing {
            if state.constraints_mode.editing {
                " EDIT CONSTRAINT — Enter to save, Esc to cancel "
            } else {
                " ADD CONSTRAINT — Enter to save, Esc to cancel "
            }
        } else {
            " CONSTRAINTS  ↑↓ navigate   d delete   a add   e edit   q quit "
        };
        Line::from(vec![Span::styled(
            label,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )])
    } else if state.processing {
        Line::from(vec![
            Span::styled(
                " thinking ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  working..."),
        ])
    } else {
        Line::from("")
    };
    frame.render_widget(separator(frame.area().width), layout[3]);
    frame.render_widget(
        Paragraph::new(status).alignment(Alignment::Right),
        layout[3],
    );

    let input_text = render_input_text(input, state);
    frame.render_widget(
        Paragraph::new(input_text).wrap(Wrap { trim: false }),
        layout[4],
    );
}

fn render_input_text(input: &TextArea<'_>, state: &UiState) -> Text<'static> {
    let input_lines = input.lines();
    if input_lines.len() == 1 && input_lines[0].is_empty() {
        if state.review_mode.active {
            return Text::from(Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    "Review mode active...",
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
        if let Some(install) = state.install_prompt.as_ref() {
            let prompt = current_install_prompt(install).unwrap_or_else(|| "value".to_owned());
            return Text::from(Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("Enter {prompt}..."),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
        if let Some(browser) = state.browser_mode.as_ref() {
            if browser.filtering {
                return Text::from(Line::from(vec![
                    Span::styled("> ", Style::default().fg(Color::Cyan)),
                    Span::styled("Filter items...", Style::default().fg(Color::DarkGray)),
                ]));
            }
            return Text::from(Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Cyan)),
                Span::styled("Menu open...", Style::default().fg(Color::DarkGray)),
            ]));
        }
        if state.constraints_mode.active
            && (state.constraints_mode.adding || state.constraints_mode.editing)
        {
            return Text::from(Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    "Enter constraint text...",
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
        return Text::from(Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "Ask Vibn to do anything...",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    let current_text = current_input_text(input);
    let slash = if state.constraints_mode.active
        || state.review_mode.active
        || state.browser_mode.is_some()
        || state.install_prompt.is_some()
    {
        None
    } else {
        slash_context(&current_text, &state.config, &state.cwd)
    };
    let mut rendered = Vec::with_capacity(input_lines.len());
    for (index, line) in input_lines.iter().enumerate() {
        if index == 0 {
            let mut spans = vec![
                Span::styled("> ", Style::default().fg(Color::Cyan)),
                Span::raw(line.clone()),
            ];
            if let Some(context) = slash.as_ref() {
                if let Some(exact) = context.exact_description.as_ref() {
                    spans.push(Span::styled(
                        format!("  {exact}"),
                        Style::default().fg(Color::DarkGray),
                    ));
                } else if let Some(suffix) = context.suggestion_suffix.as_ref() {
                    spans.push(Span::styled(
                        suffix.clone(),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }
            rendered.push(Line::from(spans));
        } else {
            rendered.push(Line::from(vec![Span::raw(format!("  {line}"))]));
        }
    }
    Text::from(rendered)
}

fn render_completion_popup(
    frame: &mut Frame<'_>,
    output_area: Rect,
    context: &SlashContext,
    completion_index: usize,
) {
    let visible = context.items.len().min(6);
    if visible == 0 || output_area.height < 4 || output_area.width < 20 {
        return;
    }
    let width = output_area.width.min(72).max(24);
    let height = (visible as u16 + 2).min(output_area.height);
    let area = Rect {
        x: output_area.x,
        y: output_area.y + output_area.height.saturating_sub(height),
        width,
        height,
    };
    let lines = context
        .items
        .iter()
        .take(visible)
        .enumerate()
        .map(|(index, item)| {
            let selected = index == completion_index.min(context.items.len() - 1);
            let prefix = if selected { "› " } else { "  " };
            let style = if selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default()
            };
            Line::from(vec![
                Span::styled(format!("{prefix}{:<24}", item.display), style),
                Span::styled(
                    item.meta.to_owned(),
                    if selected {
                        style
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
            ])
        })
        .collect::<Vec<_>>();
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().title(" commands ").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn separator(width: u16) -> Paragraph<'static> {
    Paragraph::new("─".repeat(width as usize)).style(Style::default().fg(Color::DarkGray))
}

fn current_input_text(input: &TextArea<'_>) -> String {
    input.lines().join("\n")
}

fn sync_completion_index(state: &mut UiState, input: &TextArea<'_>) {
    if let Some(context) = slash_context(&current_input_text(input), &state.config, &state.cwd) {
        if !context.items.is_empty() {
            state.completion_index = state.completion_index.min(context.items.len() - 1);
            return;
        }
    }
    state.completion_index = 0;
}

fn slash_context(text: &str, config: &AppConfig, cwd: &Path) -> Option<SlashContext> {
    if text.contains('\n') {
        return None;
    }
    if text.starts_with('!') {
        let shell_text = text[1..].trim_start();
        if shell_text.is_empty() {
            return None;
        }
        let partial = shell_text
            .split_whitespace()
            .last()
            .unwrap_or(shell_text)
            .to_owned();
        let prefix = text[..text.len().saturating_sub(partial.len())].to_owned();
        let items = complete_path_items(&prefix, &partial, cwd);
        return Some(SlashContext {
            items,
            exact_description: None,
            suggestion_suffix: None,
        });
    }
    if let Some((prefix, partial)) = trailing_file_mention(text) {
        let items = complete_path_items(&prefix, &partial, cwd);
        return Some(SlashContext {
            items,
            exact_description: None,
            suggestion_suffix: None,
        });
    }
    if !text.starts_with('/') {
        return None;
    }
    let parts = text.splitn(2, char::is_whitespace).collect::<Vec<_>>();
    let base_cmd = parts.first().copied().unwrap_or(text);
    if parts.len() > 1 && path_command(base_cmd) {
        let partial = parts[1];
        let prefix = text[..text.len().saturating_sub(partial.len())].to_owned();
        let items = complete_path_items(&prefix, partial, cwd);
        return Some(SlashContext {
            items,
            exact_description: None,
            suggestion_suffix: None,
        });
    }
    let exact = exact_command(text);
    let items = if exact.is_some() {
        Vec::new()
    } else {
        command_matches(config, text)
    };
    let suggestion_suffix = items
        .first()
        .filter(|item| item.apply_text.starts_with(text))
        .map(|item| item.apply_text[text.len()..].to_owned());
    Some(SlashContext {
        items,
        exact_description: exact.map(|definition| definition.description.to_owned()),
        suggestion_suffix,
    })
}

fn command_matches(config: &AppConfig, text: &str) -> Vec<CompletionItem> {
    let mut matches = slash_command_definitions()
        .iter()
        .copied()
        .filter(|definition| definition.command.starts_with(text))
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        config
            .command_frequency(right.command)
            .cmp(&config.command_frequency(left.command))
            .then_with(|| left.command.cmp(right.command))
    });
    matches
        .into_iter()
        .map(|definition| CompletionItem {
            apply_text: definition.command.to_owned(),
            display: definition.command.trim_end().to_owned(),
            meta: if config.command_frequency(definition.command) > 0 {
                format!(
                    "{}  ({}x)",
                    definition.description,
                    config.command_frequency(definition.command)
                )
            } else {
                definition.description.to_owned()
            },
        })
        .collect()
}

fn exact_command(text: &str) -> Option<SlashCommandDefinition> {
    slash_command_definitions()
        .iter()
        .copied()
        .find(|definition| definition.command == text)
}

fn selected_completion(context: &SlashContext, completion_index: usize) -> Option<String> {
    context
        .items
        .get(completion_index.min(context.items.len().saturating_sub(1)))
        .map(|item| item.apply_text.clone())
}

fn path_command(command: &str) -> bool {
    command == "/cd"
}

fn trailing_file_mention(text: &str) -> Option<(String, String)> {
    let at = text.rfind('@')?;
    let suffix = &text[at + 1..];
    if suffix.is_empty()
        || !suffix
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '/' | '_' | '-'))
    {
        return None;
    }
    Some((text[..at + 1].to_owned(), suffix.to_owned()))
}

fn complete_path_items(prefix_text: &str, partial: &str, cwd: &Path) -> Vec<CompletionItem> {
    let partial_expanded = if partial == "~" {
        "~/".to_owned()
    } else {
        partial.to_owned()
    };
    let partial_path = if let Some(stripped) = partial_expanded.strip_prefix("~/") {
        env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(stripped)
    } else {
        PathBuf::from(&partial_expanded)
    };
    let (base_dir, typed_prefix) = if partial_path.is_absolute() {
        (
            partial_path
                .parent()
                .unwrap_or_else(|| Path::new("/"))
                .to_path_buf(),
            partial_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_owned(),
        )
    } else {
        let full = cwd.join(&partial_path);
        (
            full.parent().unwrap_or(cwd).to_path_buf(),
            partial_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_owned(),
        )
    };
    if !base_dir.is_dir() {
        return Vec::new();
    }
    let replace_prefix = if partial.contains('/') {
        let mut path = partial.replace('\\', "/");
        let keep = path.rfind('/').map(|index| index + 1).unwrap_or(0);
        path.truncate(keep);
        path
    } else {
        String::new()
    };
    let mut entries = fs::read_dir(base_dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    entries
        .into_iter()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if matches!(
                name.as_str(),
                ".git"
                    | "node_modules"
                    | "__pycache__"
                    | "venv"
                    | ".venv"
                    | ".next"
                    | "dist"
                    | "build"
                    | ".cache"
                    | ".turbo"
            ) {
                return None;
            }
            if name.starts_with('.') && !typed_prefix.starts_with('.') {
                return None;
            }
            if !name
                .to_ascii_lowercase()
                .starts_with(&typed_prefix.to_ascii_lowercase())
            {
                return None;
            }
            let is_dir = entry.path().is_dir();
            let suffix = if is_dir { "/" } else { "" };
            Some(CompletionItem {
                apply_text: format!("{prefix_text}{replace_prefix}{name}{suffix}"),
                display: format!("{name}{suffix}"),
                meta: if is_dir {
                    "dir".to_owned()
                } else {
                    String::new()
                },
            })
        })
        .collect()
}

fn resolve_cd_path(cwd: &Path, arg: &str) -> PathBuf {
    let expanded = if let Some(stripped) = arg.strip_prefix("~/") {
        env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(stripped)
    } else {
        PathBuf::from(arg)
    };
    if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    }
}

fn summarize_tool_args(tool: &str, args: &Map<String, Value>) -> String {
    let raw = args
        .get("command")
        .and_then(Value::as_str)
        .or_else(|| args.get("args").and_then(Value::as_str))
        .map(str::to_owned)
        .unwrap_or_else(|| serde_json::to_string(args).unwrap_or_else(|_| "{}".to_owned()));
    let mut text = if tool == "git" && !raw.starts_with("git ") {
        format!("git {raw}")
    } else {
        raw
    }
    .replace('\n', " ");
    if text.len() > 120 {
        text.truncate(119);
        text.push('…');
    }
    text
}

fn render_diff_lines(path: &Path, old_content: &str, new_content: &str) -> Vec<String> {
    let mut lines = vec![
        String::new(),
        format!(
            "{}: {}",
            if old_content.is_empty() {
                "New file"
            } else {
                "Modified"
            },
            path.display()
        ),
        String::new(),
        format!("--- a/{}", path.display()),
        format!("+++ b/{}", path.display()),
    ];
    let old_lines = old_content.lines().collect::<Vec<_>>();
    let new_lines = new_content.lines().collect::<Vec<_>>();
    let limit = old_lines.len().max(new_lines.len()).min(120);
    for index in 0..limit {
        match (old_lines.get(index), new_lines.get(index)) {
            (Some(old), Some(new)) if old == new => lines.push(format!(" {old}")),
            (Some(old), Some(new)) => {
                lines.push(format!("-{old}"));
                lines.push(format!("+{new}"));
            }
            (Some(old), None) => lines.push(format!("-{old}")),
            (None, Some(new)) => lines.push(format!("+{new}")),
            (None, None) => {}
        }
    }
    if old_lines.len().max(new_lines.len()) > limit {
        lines.push(format!(
            "... {} more lines omitted ...",
            old_lines.len().max(new_lines.len()) - limit
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{
        BrowserView, ConstraintsMode, apply_config_assignment, apply_model_path_change,
        apply_permission_rule, browser_view_title, build_install_prompt_mode, clean_review_example,
        command_matches, complete_path_items, exact_command, filter_browser_items,
        first_unreviewed_index, format_gb, get_market_entry, heuristic_summary, last_agent_block,
        load_pins_from, model_picker_description, normalize_commit_message, parse_compare_command,
        parse_watch_command, pinned_notes_block, render_constraints_lines,
        render_model_storage_summary, render_project_context, resolve_skill_matches,
        skill_activation_messages, slash_context, truncate_last_turn,
    };
    use serde_json::{Map, Value};
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};
    use vibn_core::{AppConfig, ChatMessage, ModelInfo, SystemProfile};

    fn config_with_usage(values: &[(&str, u64)]) -> AppConfig {
        let mut usage = Map::new();
        for (command, count) in values {
            usage.insert((*command).to_owned(), Value::from(*count));
        }
        let mut extra = Map::new();
        extra.insert("command_usage".to_owned(), Value::Object(usage));
        AppConfig {
            schema_version: 1,
            default_model: "qwen2.5-coder:7b".to_owned(),
            ollama_models_path: String::new(),
            extra,
        }
    }

    fn temp_case_dir(label: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("vibn-{label}-{unique}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn command_matches_sort_by_usage_then_name() {
        let config = config_with_usage(&[("/model", 2), ("/memory", 5)]);
        let matches = command_matches(&config, "/m");
        assert_eq!(matches[0].apply_text, "/memory");
        assert_eq!(matches[1].apply_text, "/model");
    }

    #[test]
    fn slash_context_shows_exact_command_description_state() {
        let config = config_with_usage(&[]);
        let context = slash_context("/help", &config, Path::new(".")).expect("context");
        assert_eq!(
            context.exact_description.expect("exact"),
            "Show all commands"
        );
        assert!(context.items.is_empty());
        assert!(context.suggestion_suffix.is_none());
    }

    #[test]
    fn exact_command_matches_trailing_space_commands() {
        assert!(exact_command("/cd").is_none());
        assert_eq!(
            exact_command("/cd ").expect("exact").description,
            "Change working directory"
        );
    }

    #[test]
    fn render_constraints_marks_selected_rule() {
        let lines = render_constraints_lines(&ConstraintsMode {
            active: true,
            adding: false,
            editing: false,
            edit_index: 0,
            selected: 1,
            rules: vec!["first".to_owned(), "second".to_owned()],
        });
        assert!(lines.iter().any(|line| line.contains("▶ second")));
    }

    #[test]
    fn render_project_context_includes_prompt_and_constraints() {
        let config = serde_json::json!({
            "system_prompt": "Follow repo rules.",
            "constraints": ["Keep parity", "Do not redesign"],
        });
        let rendered = render_project_context(config.as_object().expect("object"));
        assert!(rendered.contains("Follow repo rules."));
        assert!(rendered.contains("Project constraints:"));
        assert!(rendered.contains("- Keep parity"));
    }

    #[test]
    fn first_unreviewed_index_skips_reviewed_examples() {
        let examples = vec![
            serde_json::json!({"_meta": {"reviewed": true}}),
            serde_json::json!({"_meta": {"reviewed": false}}),
            serde_json::json!({}),
        ];
        assert_eq!(first_unreviewed_index(&examples), 1);
    }

    #[test]
    fn clean_review_example_drops_meta() {
        let value = serde_json::json!({
            "_meta": {"reviewed": true},
            "conversations": [{"from": "gpt", "value": "hi"}]
        });
        let cleaned = clean_review_example(&value);
        assert!(cleaned.get("_meta").is_none());
        assert!(cleaned.get("conversations").is_some());
    }

    #[test]
    fn resolve_skill_matches_supports_unique_search() {
        let skill = resolve_skill_matches("debugging").expect("skill");
        assert_eq!(skill.key, "debug");
    }

    #[test]
    fn skill_activation_messages_match_legacy_flow() {
        let skill = resolve_skill_matches("debug").expect("skill");
        let messages = skill_activation_messages(skill);
        assert!(messages[0].content.contains("[SKILL ACTIVATED: Debug]"));
        assert!(messages[1].content.contains("I'm now in **Debug** mode"));
    }

    #[test]
    fn build_install_prompt_mode_collects_placeholder_steps() {
        let entry = get_market_entry("filesystem").expect("entry");
        let install = build_install_prompt_mode(entry);
        assert_eq!(install.resolved_args[0].as_deref(), Some("-y"));
        assert_eq!(
            install.resolved_args[1].as_deref(),
            Some("@modelcontextprotocol/server-filesystem")
        );
        assert_eq!(install.steps.len(), 1);
    }

    #[test]
    fn filter_browser_items_uses_label_description_and_meta() {
        let items = vec![
            super::BrowserItem {
                label: "github".to_owned(),
                description: "GitHub API".to_owned(),
                meta: "connected | Developer Tools".to_owned(),
                style: super::BrowserItemStyle::Connected,
                selectable: true,
                action: super::BrowserAction::None,
            },
            super::BrowserItem {
                label: "── Categories ──".to_owned(),
                description: String::new(),
                meta: String::new(),
                style: super::BrowserItemStyle::Separator,
                selectable: false,
                action: super::BrowserAction::None,
            },
        ];
        let filtered = filter_browser_items(&items, "developer");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].label, "github");
    }

    #[test]
    fn browser_view_title_includes_model_variants() {
        assert_eq!(browser_view_title(&BrowserView::ModelPicker), "Models");
        assert_eq!(
            browser_view_title(&BrowserView::StoragePicker),
            "Model Storage Path"
        );
    }

    #[test]
    fn model_picker_description_includes_summary_and_ram() {
        let info = ModelInfo {
            summary: "Fast default coding model".to_owned(),
            size_gb: 4.7,
            use_cases: vec!["coding".to_owned(), "chat".to_owned()],
            tool_support: true,
            min_ram_gb: 8,
            recommended_ram_gb: 12,
            source: "ollama".to_owned(),
            gguf: None,
        };
        let profile = SystemProfile {
            system: "macOS".to_owned(),
            machine: "arm64".to_owned(),
            cpu_count: 8,
            total_ram_gb: Some(24.0),
            storage_path: Path::new("/tmp/models").to_path_buf(),
            storage_free_gb: Some(100.0),
        };
        let description = model_picker_description("qwen2.5-coder:7b", &info, &profile);
        assert!(description.contains("Fast default coding model"));
        assert!(description.contains("12GB+ RAM"));
    }

    #[test]
    fn format_gb_matches_expected_precision() {
        assert_eq!(format_gb(Some(120.0)), "120GB");
        assert_eq!(format_gb(Some(12.34)), "12.3GB");
        assert_eq!(format_gb(Some(4.567)), "4.57GB");
        assert_eq!(format_gb(None), "unknown");
    }

    #[test]
    fn complete_path_items_lists_matching_directories_with_trailing_slash() {
        let temp = temp_case_dir("path-complete");
        let crate_dir = temp.join("crates");
        let src_file = temp.join("src.txt");
        fs::create_dir_all(crate_dir.join("vibn")).expect("create nested dir");
        fs::write(&src_file, "hello").expect("write file");

        let items = complete_path_items("/cd ", "cr", &temp);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].apply_text, "/cd crates/");
        assert_eq!(items[0].display, "crates/");

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn slash_context_uses_path_completion_for_cd_arguments() {
        let config = config_with_usage(&[]);
        let temp = temp_case_dir("slash-context");
        fs::create_dir_all(temp.join("src")).expect("create src dir");

        let context = slash_context("/cd s", &config, &temp).expect("context");
        assert!(context.exact_description.is_none());
        assert_eq!(context.items.len(), 1);
        assert_eq!(context.items[0].apply_text, "/cd src/");

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn truncate_last_turn_removes_last_user_and_followups() {
        let messages = vec![
            ChatMessage::user("first"),
            ChatMessage::assistant("one"),
            ChatMessage::user("second"),
            ChatMessage::tool("read_file", "result"),
            ChatMessage::assistant("two"),
        ];
        let trimmed = truncate_last_turn(&messages).expect("trimmed");
        assert_eq!(trimmed.len(), 2);
        assert_eq!(trimmed[0].content, "first");
        assert_eq!(trimmed[1].content, "one");
    }

    #[test]
    fn apply_config_assignment_updates_known_and_extra_keys() {
        let mut config = config_with_usage(&[]);
        let message =
            apply_config_assignment(&mut config, r#"default_model="llama3.2:3b""#).expect("set");
        assert_eq!(message, r#"default_model = "llama3.2:3b""#);
        assert_eq!(config.default_model, "llama3.2:3b");

        apply_config_assignment(&mut config, "command_timeout=45").expect("extra");
        assert_eq!(
            config.extra.get("command_timeout").and_then(Value::as_i64),
            Some(45)
        );
    }

    #[test]
    fn apply_model_path_change_resolves_relative_paths() {
        let temp = temp_case_dir("model-path");
        let mut config = config_with_usage(&[]);
        let message = apply_model_path_change(&mut config, &temp, "models/thumb").expect("path");
        assert!(message.contains("Model storage path set to"));
        assert!(config.ollama_models_path.ends_with("models/thumb"));
        assert!(Path::new(&config.ollama_models_path).is_dir());
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn render_model_storage_summary_includes_path_label() {
        let mut config = config_with_usage(&[]);
        config.ollama_models_path = "/tmp/vibn-models".to_owned();
        let summary = render_model_storage_summary(&config);
        assert!(summary.contains("Model storage path: /tmp/vibn-models"));
        assert!(summary.contains("Storage:"));
    }

    #[test]
    fn heuristic_summary_collapses_tool_activity() {
        let mut assistant = ChatMessage::assistant("");
        assistant.tool_calls = vec![vibn_core::ToolCall {
            function: vibn_core::ToolFunction {
                name: "read_file".to_owned(),
                arguments: Map::new(),
            },
        }];
        let messages = vec![
            ChatMessage::user("inspect config"),
            assistant,
            ChatMessage::tool("read_file", "1: hello\n2: world"),
        ];
        let summary = heuristic_summary(&messages);
        assert!(summary.contains("User: inspect config"));
        assert!(summary.contains("Agent used read_file"));
    }

    #[test]
    fn pinned_notes_block_renders_prompt_section() {
        let block = pinned_notes_block(&["remember this".to_owned(), "and this".to_owned()])
            .expect("block");
        assert!(block.contains("## Pinned Notes (always remember)"));
        assert!(block.contains("- remember this"));
        assert!(block.contains("- and this"));
    }

    #[test]
    fn load_pins_from_reads_text_entries() {
        let temp = temp_case_dir("pins-load");
        let path = temp.join("pins.json");
        fs::write(&path, r#"[{"text":"first"},{"text":"second"}]"#).expect("write pins");
        let pins = load_pins_from(&path).expect("load pins");
        assert_eq!(pins, vec!["first".to_owned(), "second".to_owned()]);
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn apply_permission_rule_writes_command_match_rule() {
        let mut config = config_with_usage(&[]);
        let message = apply_permission_rule(&mut config, "/allow", "run_command npm test")
            .expect("permission");
        assert_eq!(message, "always_allow: run_command (npm test)");
        let rules = config
            .extra
            .get("permissions")
            .and_then(Value::as_object)
            .and_then(|permissions| permissions.get("always_allow"))
            .and_then(Value::as_array)
            .expect("rules");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0]["tool"], "run_command");
        assert_eq!(rules[0]["commands"][0], "npm test");
    }

    #[test]
    fn apply_permission_rule_writes_path_match_rule() {
        let mut config = config_with_usage(&[]);
        let message =
            apply_permission_rule(&mut config, "/deny", "write_file .env*").expect("permission");
        assert_eq!(message, "always_deny: write_file (.env*)");
        let rules = config
            .extra
            .get("permissions")
            .and_then(Value::as_object)
            .and_then(|permissions| permissions.get("always_deny"))
            .and_then(Value::as_array)
            .expect("rules");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0]["tool"], "write_file");
        assert_eq!(rules[0]["paths"][0], ".env*");
    }

    #[test]
    fn last_agent_block_skips_user_tool_and_shell_lines() {
        let lines = vec![
            "> user prompt".to_owned(),
            "[tool:read_file] some result".to_owned(),
            "Assistant line one".to_owned(),
            "Assistant line two".to_owned(),
            String::new(),
            "!git status".to_owned(),
        ];
        let block = last_agent_block(&lines).expect("block");
        assert_eq!(block, "Assistant line one\nAssistant line two");
    }

    #[test]
    fn normalize_commit_message_strips_code_fences_and_whitespace() {
        let message = normalize_commit_message("```fix bug\n\n- keep detail```\n");
        assert_eq!(message, "fix bug\n\n- keep detail");
    }

    #[test]
    fn parse_compare_command_supports_optional_model_prefix() {
        let parsed = parse_compare_command("qwen2.5-coder:14b: write a test", "qwen2.5-coder:7b")
            .expect("parsed");
        assert_eq!(parsed.0, "qwen2.5-coder:14b");
        assert_eq!(parsed.1, "write a test");
    }

    #[test]
    fn parse_watch_command_uses_default_prompt() {
        let parsed = parse_watch_command("src").expect("parsed");
        assert_eq!(parsed.0, "src");
        assert!(parsed.1.contains("{path}"));
    }
}
