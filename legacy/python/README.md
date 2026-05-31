# Removed Python Implementation

The pre-Rust Vibn implementation was removed after its runtime behavior was ported to Rust.

This directory is intentionally documentation-only:

- `./vibn` no longer falls back to Python.
- New behavior should be implemented in `crates/vibn` or `crates/vibn-core`.
- Do not add Python runtime files here. If legacy behavior is needed, port it to Rust.

Migration map:

- CLI entry point -> `crates/vibn/src/main.rs`
- TUI and slash commands -> `crates/vibn/src/tui.rs`
- Agent loop, tools, config, permissions, hooks, tokens, transcripts, observations, MCP -> `crates/vibn-core/src/lib.rs`
- Training export/generation/validation -> Rust slash commands in `crates/vibn/src/tui.rs`
