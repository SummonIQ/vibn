# Copilot Instructions

## Commands

- Install dependencies: `cargo build --release && cargo run --release`
- Run the CLI directly: `python3 main.py`
- Run through the repo wrapper script (activates `./venv` first): `./vibn`
- List recommended Ollama models: `python3 main.py --list-models`
- Run a one-off non-interactive prompt: `python3 main.py "your prompt"`
- Export training-data stats from saved transcripts: `python3 training/export_data.py --stats`
- Export training data: `python3 training/export_data.py`
- Inspect the training-data generator CLI: `python3 training/generate_data.py --help`
- There is no committed repo-local test, lint, or build command today; do not invent one in changes or docs without adding the underlying tooling.

## High-level architecture

- `main.py` is the entry point. It parses CLI flags, then hands off to `tui.run_tui(...)`. The `vibn` shell script is just a thin wrapper that activates `./venv` and runs `python main.py`.
- `tui.py` is the orchestrator. It owns the fullscreen prompt_toolkit app, slash commands, transcript session loading/saving, model checks, background test loop state, diff/permission confirmation UI, plugin loading, MCP auto-connect, and initial project indexing.
- `agent.py` is the execution engine behind each chat turn. It maintains the message history and system prompt, streams Ollama responses, retries failed model calls, auto-compacts when token budget is tight, parses fallback text-form tool calls from smaller models, and dispatches built-in plus MCP tools.
- `tools.py` is the built-in tool layer. It exposes working-directory-aware file, search, shell, git, and observation tools, and it is also the registration point for plugin tools loaded from `~/.vibn/plugins/*.py`.
- Project context is built in `indexer.py`. On startup and `/cd`, the TUI indexes the target directory, summarizes the tree and git state, and injects instruction files directly into the agent prompt. `indexer.py` already treats `.github/copilot-instructions.md` as one of the instruction sources.
- User state is intentionally outside the repo. `config.py`, `permissions.py`, `hooks.py`, `transcripts.py`, `observations.py`, and `memory.py` all read and write under `~/.vibn/` for config, permissions, hooks, transcripts, observations, plugins, and remembered facts.
- MCP support is centralized in `mcp_client.py`. Connected servers expose tools under the `mcp__<server>__<tool>` naming convention, and `tui.py` can auto-connect servers from `~/.vibn/config.json`.
- `training/` is a separate pipeline that consumes transcript data from `~/.vibn/transcripts/`, exports ShareGPT-style data, and documents the fine-tuning workflow in `training/README.md`.

## Key conventions

- `AGENTS.md` is the primary repo-authored behavior guide. Its rules are strict: make only requested changes, avoid drive-by refactors, and do not add comments, docstrings, or type hints unless the surrounding file already uses them consistently.
- Relative paths in tool execution resolve through the global working directory managed in `tools.py`, not through ad hoc `os.getcwd()` calls. `/cd` updates that tool working directory and also refreshes indexed project context.
- If you add or change built-in tools, keep `TOOL_DEFINITIONS` and `TOOL_HANDLERS` in sync in `tools.py`. Runtime-loaded plugin tools follow a different path: they come from `~/.vibn/plugins/*.py` and are registered through `register_plugin_tools(...)`.
- Permission checks are centralized. Shell-command confirmation and protected-path handling belong in `permissions.py` and `config.py`; do not add scattered destructive-command checks elsewhere.
- Project-specific runtime customization belongs in a project-local `.vibn` JSON file, not in source modules. `tui.py` reads `.vibn` for `system_prompt`, `constraints`, `model`, and `test_cmd`, and `/test` reuses that stored command.
- Instruction files are part of runtime behavior, not just docs. `indexer.py` injects `AGENTS.md`, `CLAUDE.md`, `.cursorrules`, `.github/copilot-instructions.md`, `CONVENTIONS.md`, and `CONTRIBUTING.md` into the system prompt when present, so edits to those files immediately affect agent behavior.
- Transcript, observation, and memory formats are shared across multiple subsystems. `agent.py` appends transcript entries during a chat, `tui.py` saves the full transcript on exit, `training/export_data.py` consumes transcript files, and observations/memory are also injected back into prompts. Avoid changing these storage formats casually.
