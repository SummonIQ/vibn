# Vibn — Local AI Coding Agent

A terminal-based coding agent powered by Ollama (local LLMs). The active implementation is Rust: a `clap` CLI, Ratatui fullscreen TUI, MCP protocol support, and a built-in tool ecosystem.

## Monorepo layout

This repo holds both the Rust agent and its JS/TS surfaces:

- `crates/vibn-core/` — core agent loop (Rust library)
- `crates/vibn/` — CLI binary (Rust)
- `crates/vibn-desktop/` — Tauri desktop app (Rust)
- `apps/marketing/` — Next.js marketing site, deployed to https://vibn.dev
- `apps/api/` — Next.js API (better-auth + Prisma + Neon), WIP

Rust workspace is the root `Cargo.toml`. JS/TS workspace is the root `package.json` with `workspaces: ["apps/*"]` (Bun). Build orchestration via Turborepo (`turbo.json`).

Brand assets live at `apps/marketing/public/brand/`. The desktop app uses the same mark from `crates/vibn-desktop/icons/`.

## Architecture

### Key Files

- `vibn` — Rust-only launcher. Builds `crates/vibn` and executes `target/debug/vibn`.
- `crates/vibn/src/main.rs` — CLI entry point and non-interactive prompt/agent modes.
- `crates/vibn/src/tui.rs` — Ratatui fullscreen app, slash commands, completion, marketplace/skills/model browsers, background tasks, training-data review/generation/export.
- `crates/vibn-core/src/lib.rs` — Core agent loop, Ollama client, tool dispatch, file state cache, token budget tracking, auto-compact, hooks, permissions, transcripts, observations, MCP client, model registry, and built-in tools.
- `data/default_config.json` — Default config merged into `~/.vibn/config.json`.
- `data/models.json` — Model registry used by model listing and recommendations.
- `training/seeds.json` — Seed templates for Rust-native `/generate-training-data`.
- `training/Modelfile` and `training/vibn_finetune.ipynb` — Fine-tuning assets.
- `legacy/python/README.md` — Documents that the old Python implementation was removed after the Rust migration.

### Data Directories

- `~/.vibn/config.json` — Main config (model, permissions, hooks, MCP servers, command usage)
- `~/.vibn/observations/` — Global observations + `projects/<slug>/OBSERVATIONS.md` per project
- `~/.vibn/transcripts/` — JSONL session transcripts
- `~/.vibn/history` — Command history for the TUI
- `~/.vibn/sessions/` — Session storage

### Tech Stack

Rust 2024, `clap`, `ratatui`, `crossterm`, `tui-textarea`, `reqwest`, `rmcp`, `serde`, `tokio`, `regex`.

## Agent Behavior

- The system prompts in `crates/vibn/src/main.rs` and `crates/vibn/src/tui.rs` define Vibn as a local AI coding agent with filesystem and shell tool access.
- Small Ollama models often emit tool calls as text. `parse_tool_calls_from_text()` in `vibn-core` intercepts these and executes them. It handles `{"name": "tool", "arguments": {...}}` JSON and `tool_name {args}` inline patterns.
- The core agent loop is in `run_agent_turns_with_callbacks()` in `crates/vibn-core/src/lib.rs`.
- Token budget auto-compacts at 90% of context window. Smart compaction collapses tool call/result pairs into one-liner summaries.
- File reads are cached per turn via `FileStateCache` and invalidated on writes/edits.
- All tool calls go through Rust permission checks before execution.
- Hooks fire on session, compaction, edit, command, and chat events.

## Conventions

- Only make changes the user explicitly asks for.
- Don't add comments, docstrings, or type annotations to code you didn't write.
- Keep the TUI responsive — long operations run in background threads and communicate through `UiEvent`.
- Slash commands are handled in `execute_slash_command()` in `crates/vibn/src/tui.rs`.
- Do not reintroduce Python as a runtime dependency. If legacy behavior is needed, port it to Rust.

## Codex Execution Discipline

### Universal Execution Rules
- Restate the exact defect in one sentence before editing.
- Classify the issue first: UI, runtime/state, data/persistence, API/integration, or workflow/orchestration.
- Trace the issue from trigger to final side effect before changing code.
- Identify the single subsystem most likely responsible before the first edit.
- First attempt: make one direct change in that subsystem only.
- Do not broaden scope because of uncertainty.
- Do not treat uncertainty as permission to touch nearby code.
- Do not mix cleanup, refactors, redesign, or speculative improvements into a targeted fix unless explicitly requested.
- If the obvious cause is visible from the screenshot, error, or failing behavior, start with that obvious cause first.
- If the first fix misses, reset to the traced path and patch the next direct cause only.
- After two misses, stop broad inference and instrument the path or use only literal user-specified deltas.
- Report scope explicitly after each patch with `Changed:` and `Did not change:`.

### Scope Discipline
- Prefer literal interpretation over inferred intent for bug-fix tasks.
- For narrow tasks, optimize for correctness and scope control before initiative.
- If the user is already narrowing scope, reduce initiative further and stick to literal deltas.
- Do not convert a precise task into a cleanup pass or adjacent enhancement.

### Screenshot-Driven UI Rules
- Identify the single visible defect in plain language before editing.
- Change the most direct visual cause first.
- Prefer outer spacing/container issues before inner alignment issues when the screenshot shows excess space around the content.
- On the first attempt, change exactly one class or one property.
- Do not change neighboring controls, icon sizes, font sizes, or unrelated layout unless explicitly requested.

### Component Reuse Rules
- For UI work, check whether an existing shared component already solves the problem before creating anything new.
- If the project uses shadcn/ui or shared design-system components, prefer those first.
- Inspect the existing component inventory and local patterns before adding a new component.
- Do not recreate common primitives from scratch when an existing shadcn/ui, shared, or project-local component already exists.
- Extend existing components before inventing new parallel versions unless the existing one is clearly the wrong abstraction.
- Match existing component APIs and styling conventions instead of introducing one-off patterns.
- If a new component is truly necessary, state why the existing components are insufficient before adding it.
- On UI bug fixes, patch the nearest existing component or usage site first instead of creating a replacement component.

### Runtime And State Rules
- For runtime/state bugs, trace the real execution path before editing.
- Start at the failing boundary: event handler, runner, reducer/store, effect, IPC handler, API route, or persistence layer.
- Fix the first broken handoff in the path rather than editing UI around it.
- If state reverts, inspect overwrite/reload paths before changing setters.
- If iteration/orchestration fails, inspect the executor before changing the steps or prompts.

### Data And Persistence Rules
- For data bugs, trace: input -> transform -> validation -> write -> readback -> render.
- If data is missing after a write, inspect the payload and overwrite path first.
- Do not change schemas, types, and UI together on the first pass unless the traced failure requires it.

### JavaScript And TypeScript Rules
- Trace behavior through the actual call path instead of editing types first.
- Use types to confirm intent, not as an excuse for speculative cleanup.
- Do not widen or refactor types unless the traced bug requires it.
- Check the concrete runtime payloads and template resolution path before changing helper abstractions.

### React Rules
- Localize the issue to props, state, effects, derived render logic, or component boundaries before editing.
- Do not restructure components, rename props, or restyle siblings on a targeted bug fix unless required.
- For visual issues, change the nearest responsible element first.
- For interaction bugs, trace the event -> state -> render path first.

### Next.js Rules
- Trace through the real Next.js boundary first: route segment, server component, client component, server action, API route, cache layer, or middleware.
- Do not change caching, data fetching, and component structure together on the first attempt.
- For hydration or boundary issues, verify server/client ownership before editing markup or state.
- For persistence issues, inspect request payloads and revalidation/reload behavior before changing the UI.

### Vite Rules
- Start with the real failing boundary: module resolution, env loading, HMR path, plugin transform, or build config.
- Do not rewrite config broadly when the traced failure is local to one entry, alias, or plugin.

### Electron, Electrobun, And Tauri Rules
- Trace the issue across the real process boundary first: renderer, preload/bridge, IPC channel, main process, native shell, or webview.
- Do not edit renderer UI first when the failure may be in IPC, host message delivery, or native window state.
- For embedded browser issues, inspect state exclusivity and container spacing before changing controls or styling around them.


404: Not Found
