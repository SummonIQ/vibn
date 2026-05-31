# Vibn Cloud Migration Final Plan

This is the single source-of-truth plan for migrating Vibn from a local-first Rust coding agent to a hosted API/cloud product while preserving local execution as the primary path where it is available.

It consolidates the prior Codex and Claude planning docs, Claude's final plan in `api-cloud-plan-final.md`, and the product directive that AppLab must execute locally when possible and use cloud only as fallback. Where the earlier plans conflicted, this plan generally follows Claude's final decisions unless there is a concrete implementation reason to adjust them.

## 0. Final Decisions

These are the locked architecture decisions.

1. **AppLab is local-first with cloud fallback.**
   Resolution order per request:
   1. local Vibn CLI on the same machine
   2. paired local Vibn worker connected through the cloud control plane
   3. hosted cloud sandbox

   Cloud is a fallback for AppLab-managed local projects, not the default route.

2. **Frontends stay on Vercel.**
   `apps/marketing` remains on Vercel. A future `apps/dashboard` should also deploy to Vercel for account UI, billing, API key management, docs, and a web playground. No long-running agent execution runs on Vercel Functions.

3. **The consolidated API server runs on Railway.**
   The existing `apps/api` Next.js 15 + Better Auth + Prisma + Neon app ports from Vercel to Railway as a Next.js standalone Docker container. It keeps Better Auth and Prisma as-is, then grows `/v1/*` API routes for sessions, runs, tools, models, workers, API keys, memory, artifacts, and OpenAPI.

4. **`vibn-worker` is a separate Rust container.**
   `vibn-worker` runs the agent loop and talks to `vibn-api` over an internal channel. It starts on Railway. Move or add Fly Machines later only if region pinning, machine lifecycle control, or stateful workspace behavior justifies it.

5. **The first cloud sandbox primitive is Vercel Sandbox.**
   Use Vercel Sandbox for per-execution Firecracker-style isolation because it avoids maintaining a separate machine lifecycle system during the first public cloud workspace milestone. Revisit Fly ephemeral Machines later if persistent stateful workspaces or regional pinning become product requirements.

6. **The first queue is Postgres `FOR UPDATE SKIP LOCKED`.**
   Use Postgres-backed run claiming for low-volume cloud-sandbox jobs. Paired workers are dispatched directly over their existing outbound WebSocket. Add Redis Streams, Inngest, NATS, SQS, or another queue only after sustained queue pressure proves it is needed.

7. **The hosted model layer is OpenAI-compatible HTTP through Vercel AI Gateway, with BYOK at launch.**
   Ollama remains first-class for local CLI, desktop, and local worker modes. Hosted mode uses Vercel AI Gateway or another OpenAI-compatible gateway so model routing, BYOK, fallback, and observability do not become custom infrastructure early.

8. **Core hosted work requires three `vibn-core` seams first: `ModelBackend`, `Workspace`, and `ToolHost`.**
   Do not build a public cloud agent surface by wrapping the current CLI or by dispatching tools directly from global process state.

9. **Run events and tool-call audit logs ship in the first API milestone.**
   Durable run events, approval records, denied tool calls, redacted args/output, and artifact pointers are not post-launch hardening; they are required infrastructure for any remote or relayed execution.

10. **Cloud write tools and shell commands are not enabled by default.**
    Cloud sandbox execution begins read-only or diff-preview only. Write tools, shell commands, git mutation, MCP, network egress, ComfyUI, and large downloads require explicit policy and audit controls.

## 1. Current State

### Vibn Repo

Working repo: `/Users/steven/Projects/vibn`.

Current layout:

```text
crates/
  vibn-core/          Rust library: agent loop, Ollama, tools, MCP, hooks,
                      transcripts, observations, ComfyUI/image/video tools
  vibn/               clap CLI + Ratatui TUI
  vibn-desktop/       Tauri desktop shell invoking vibn-core through commands

apps/
  marketing/          Next.js marketing site
  api/                Next.js 15 + Better Auth + Prisma + Neon auth API

data/
  default_config.json
  models.json

training/
  seeds and fine-tuning assets
```

Important current facts:

- `crates/vibn-core/src/lib.rs` is the actual runtime. It owns `OllamaClient`, `run_agent_turns_with_callbacks`, `execute_tool_with_callbacks`, built-in tools, MCP dispatch, config loading, transcripts, observations, and ComfyUI helpers.
- `run_agent_turns_with_callbacks` currently takes an `OllamaClient`, `model`, `messages`, `AppConfig`, and `cwd`, then directly executes tools against local filesystem/process state.
- `execute_tool_with_callbacks` currently dispatches tools like `read_file`, `write_file`, `edit_file`, `patch_file`, `run_command`, `git`, `save_observation`, `read_observations`, `read_image`, `read_video`, `generate_image`, `generate_video`, `install_comfy`, `download_image_model`, and `mcp__*`.
- `vibn-core` currently assumes `~/.vibn` for config, transcripts, observations, sessions, and project memory.
- `apps/api` is not empty. It is already the Better Auth API for Vibn desktop, backed by Prisma/Neon, with trusted Tauri origins.
- `crates/vibn-desktop/src/auth.rs` currently points at `https://vibn-auth-api.vercel.app`; this must be migrated carefully to the final `api.vibn.dev` Railway deployment.

### AppLab Integration

AppLab currently calls local Vibn:

- `applab/apps/api/app/api/projects/[name]/vibn/chat/route.ts`
- `applab/apps/api/lib/vibn/local-vibn.ts`

Current behavior:

- resolves a project path
- ensures it is under `PROJECTS_BASE`
- spawns `~/Projects/vibn/vibn --cd <projectPath> <prompt>`
- supports modes `agent`, `coding`, and `chat`
- respects `VIBN_CLI_PATH`
- truncates stdout/stderr
- returns a JSON chat reply

This current local execution path is useful and should not be replaced by cloud execution when it is healthy.

### Gimme Job Patterns To Reuse

Gimme Job is not a direct Vibn dependency, but it has patterns worth copying:

- `apps/jobs-api/src/index.ts`: small public API, `/health`, `/openapi.json`, versioned `/v1/*` routes.
- `apps/jobs-api/src/rapidapi.ts`: plan headers, proxy secrets, limit caps, explicit query limits.
- `lib/desktop-tokens/index.ts`: pairing code, one-time raw token, hashed token storage, scopes, last-used tracking, revocation.

For Vibn, reuse the patterns, not the domain logic.

## 2. Target Architecture

```text
┌───────────────────────────────────────────────────────────────────────┐
│ Vercel Frontends                                                       │
│                                                                       │
│ apps/marketing   vibn.dev / vibn.com marketing                        │
│ apps/dashboard   app.vibn.dev account UI, billing, API keys, docs,    │
│                  web playground                                       │
└───────────────────────────────────────────────────────────────────────┘
                              │
                              │ HTTPS
                              ▼
┌───────────────────────────────────────────────────────────────────────┐
│ Railway: vibn-api                                                     │
│ Next.js standalone Docker container                                   │
│                                                                       │
│ Existing: Better Auth + Prisma + Neon                                 │
│ New: /v1/sessions, /v1/agent-runs, /v1/projects, /v1/tools,           │
│      /v1/models, /v1/memory, /v1/artifacts, /v1/workers/*,            │
│      /openapi.json                                                    │
│                                                                       │
│ SSE: GET /v1/agent-runs/:id/events                                    │
│ Worker channel: WSS /v1/workers/connect                               │
│ Queue: Postgres SKIP LOCKED for cloud sandbox jobs                    │
└───────────────────────────────────────────────────────────────────────┘
          │                         │                          │
          │ DB reads/writes         │ dispatch                  │ relay
          ▼                         ▼                          ▼
┌──────────────────┐      ┌───────────────────────┐   ┌──────────────────────┐
│ Neon Postgres    │      │ Railway: vibn-worker  │   │ AppLab / SDK / CLI   │
│ users, auth,     │◄────►│ Rust container        │   │ local tool execution │
│ API keys, runs,  │      │ vibn-core driver      │   └──────────────────────┘
│ run_events,      │      │                       │
│ tool_calls,      │      │ Execution backends:   │
│ artifacts, usage │      │ - RelayToolHost       │
│ memory           │      │ - SandboxToolHost     │
└──────────────────┘      │ - LocalWorker route   │
                          └───────────────────────┘
                                      │
                                      │ model calls / artifact writes
                                      ▼
                        ┌──────────────────────────────┐
                        │ Vercel AI Gateway            │
                        │ OpenAI-compatible providers  │
                        │ BYOK-first                   │
                        └──────────────────────────────┘
                                      │
                                      ▼
                        ┌──────────────────────────────┐
                        │ R2 / S3 / Blob storage       │
                        │ transcripts, logs, artifacts │
                        └──────────────────────────────┘
```

## 3. Execution Backends

All execution modes sit behind the same run/session API. Clients declare a preferred backend order, and the API selects the first available backend.

### Backend A: `local-cli`

Tools execute in the caller's local process environment by invoking the Vibn CLI.

Use cases:

- AppLab on a machine where Vibn is installed
- developer workflows where local files are the source of truth
- fastest migration path from current AppLab behavior

Rules:

- This is AppLab priority 1.
- Preserve the existing `PROJECTS_BASE` containment.
- Preserve stdout/stderr truncation.
- Preserve local permission prompts and local config behavior.
- Do not route to cloud when local CLI is healthy just to avoid a subprocess.

### Backend B: `paired-worker`

Tools execute on a user's machine via `vibn worker start`, connected outbound to the cloud API.

Use cases:

- browser/dashboard/mobile clients that need to operate on private local projects
- AppLab if local CLI is unavailable but a paired worker is online
- future no-inbound-network local automation

Rules:

- Pairing uses a short code and a one-time token exchange.
- Worker token is scoped and revocable.
- Worker opens outbound WSS to `api.vibn.dev`.
- API dispatches runs over the open worker connection.
- Worker enforces local path policy, approval prompts, command policy, and redaction.

### Backend C: `cloud-sandbox`

Tools execute in an isolated hosted workspace managed by `vibn-worker`, initially backed by Vercel Sandbox.

Use cases:

- AppLab fallback if no local option exists
- public repo demos
- web playground
- no-install onboarding
- explicit cloud execution chosen by user

Rules:

- Start read-only or diff-preview.
- Do not enable unrestricted shell commands.
- Do not enable arbitrary MCP.
- Do not run `install_comfy` or `download_image_model`.
- Use an egress allowlist.
- Enforce workspace TTL and size limits.
- Persist run events and tool-call audit logs.

### Backend Selection Contract

The run creation request should include the allowed backend order:

```json
{
  "session_id": "sess_...",
  "prompt": "Fix the failing test",
  "backends": ["local-cli", "paired-worker", "cloud-sandbox"],
  "project": {
    "kind": "local-path",
    "path": "/Users/steven/Projects/example"
  },
  "model": "openai/gpt-4o-mini"
}
```

The API response and run events must expose the backend that actually ran:

```json
{
  "run_id": "run_...",
  "backend": "local-cli",
  "status": "running"
}
```

## 4. AppLab Migration Policy

AppLab's migration is not "replace local with cloud." It is "abstract execution provider, keep local first."

### Provider Order

AppLab must resolve providers in this order:

1. `local-cli`
2. `paired-worker`
3. `cloud-sandbox`

### `localVibnAvailable()` Predicate

Availability should be checked conservatively:

```ts
async function localVibnAvailable(projectPath: string): Promise<boolean> {
  const binPath = process.env.VIBN_CLI_PATH ?? defaultVibnPath();
  if (!existsSync(binPath)) return false;

  const version = await runWithTimeout(`${binPath} --version`, 2000);
  if (!version.ok || !version.stdout.trim()) return false;

  if (!isPathUnder(projectPath, PROJECTS_BASE)) return false;

  return true;
}
```

Cache the result by `(binPath, projectPath)` for the Node process lifetime. Bust cache on explicit admin refresh or process restart. Do not require a minimum version at launch unless the new API requires a specific CLI capability.

### AppLab Provider Interface

Replace direct calls to `runLocalVibn()` with an interface:

```ts
interface VibnExecutionProvider {
  name: 'local-cli' | 'paired-worker' | 'cloud-sandbox';
  available(input: VibnRequest): Promise<boolean>;
  run(input: VibnRequest): Promise<VibnRunResult>;
}
```

The current `runLocalVibn()` becomes the implementation of `local-cli`.

### Cloud Fallback Rules

If AppLab falls back to cloud:

- the UI must expose that the run is remote
- project source must be git URL, upload/export, or explicit cloud workspace reference
- cloud should begin read-only or diff-preview
- no cloud worker should be allowed to request arbitrary local paths through AppLab
- write/shell operations must require explicit user approval events

## 5. Core Runtime Refactors

The hosted system depends on three Rust-side seams in `vibn-core`.

### 5.1 `ModelBackend`

Purpose: decouple the agent loop from `OllamaClient`.

Current problem:

- `run_agent_turns_with_callbacks` accepts `&OllamaClient`.
- Hosted runs should use OpenAI-compatible providers.
- Local runs should keep Ollama.

Target:

```rust
pub trait ModelBackend: Send + Sync {
    fn chat_message(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<Value>>,
    ) -> Result<ChatMessage, Box<dyn std::error::Error>>;
}
```

Initial implementations:

- `OllamaBackend`
- `OpenAICompatibleBackend`
- `VercelAiGatewayBackend` if a separate wrapper is useful

Important details:

- Move `OLLAMA_HOST` to config, not ambient process-only state.
- Preserve retry behavior.
- Preserve local model registry behavior for CLI/desktop.
- Add hosted model metadata through `/v1/models`.

### 5.2 `Workspace`

Purpose: replace ambient `~/.vibn` process-global state with explicit per-session/per-run context.

Current problem:

- config paths, transcript paths, observation paths, sessions, project memory, MCP registry, and ComfyUI dirs are local/global.
- multiple hosted users cannot share that process-global state safely.

Target:

```rust
pub struct Workspace {
    pub user_id: UserId,
    pub org_id: Option<OrgId>,
    pub session_id: SessionId,
    pub run_id: RunId,
    pub project_id: Option<ProjectId>,
    pub execution_mode: ExecutionMode,
    pub cwd: PathBuf,
    pub config: AppConfig,
    pub model: Arc<dyn ModelBackend>,
    pub tools: Arc<dyn ToolHost>,
    pub observations: Arc<dyn ObservationStore>,
    pub transcripts: Arc<dyn TranscriptStore>,
    pub artifacts: Arc<dyn ArtifactStore>,
    pub mcp_policy: McpPolicy,
    pub execution_policy: ExecutionPolicy,
}
```

Stores:

- `ObservationStore`: local filesystem or Postgres
- `TranscriptStore`: local filesystem or object storage
- `ArtifactStore`: local filesystem or R2/S3/Blob

Rule:

- local CLI/desktop behavior must remain unchanged through a `LocalWorkspace` default
- cloud and paired-worker behavior must pass explicit `Workspace`

### 5.3 `ToolHost`

Purpose: route every tool call through policy-aware execution.

This supersedes a filesystem-only abstraction because Vibn tools include more than file reads/writes:

- shell commands
- git
- MCP
- observations
- image/video analysis
- ComfyUI generation
- installation/download helpers

Target:

```rust
pub trait ToolHost: Send + Sync {
    fn execute_tool(
        &self,
        name: &str,
        args: &Map<String, Value>,
        context: &ToolExecutionContext,
    ) -> Result<ToolExecutionResult, ToolExecutionError>;
}
```

Implementations:

- `LocalToolHost`: current CLI/desktop behavior
- `ReadOnlyToolHost`: `read_file`, `list_directory`, `search_code`, `find_files`, selected `read_observations`
- `SandboxToolHost`: cloud workspace execution with denylist/allowlist
- `RelayToolHost`: emits tool requests to AppLab, SDK, or paired worker and waits for result

Policy dimensions:

- path containment
- command allow/deny
- max command duration
- max output bytes
- max workspace bytes
- network egress
- write approval
- git mutation approval
- MCP allowlist
- ComfyUI restrictions
- artifact limits

### 5.4 Streaming Driver

Purpose: convert `run_agent_turns_with_callbacks` into a streamable, durable worker flow.

Target crate:

- `crates/vibn-server`

Responsibilities:

- drive the agent loop
- emit run events
- persist run events before streaming them
- convert tool callbacks into `approval_required`, `diff_preview`, and `tool_call_denied`
- support cancellation
- support reconnect with cursor
- write transcript and artifacts through `Workspace`

## 6. API Surface

OpenAPI is the source of truth. Gimme Job's `/openapi.json` pattern is the reference.

### Public Endpoints

Health and spec:

- `GET /health`
- `GET /openapi.json`

Auth/API keys:

- `POST /v1/api-keys`
- `GET /v1/api-keys`
- `DELETE /v1/api-keys/{id}`

Sessions:

- `POST /v1/sessions`
- `GET /v1/sessions/{id}`
- `GET /v1/sessions/{id}/messages`
- `PATCH /v1/sessions/{id}`

Runs:

- `POST /v1/agent-runs`
- `GET /v1/agent-runs/{id}`
- `GET /v1/agent-runs/{id}/events`
- `POST /v1/agent-runs/{id}/cancel`
- `POST /v1/agent-runs/{id}/tool-results`
- `POST /v1/agent-runs/{id}/approvals`

Projects:

- `POST /v1/projects/import`
- `GET /v1/projects/{id}`
- `GET /v1/projects`

Workers:

- `POST /v1/workers/pair`
- `POST /v1/workers/exchange`
- `GET /v1/workers`
- `DELETE /v1/workers/{id}`
- `GET /v1/workers/connect` or `WSS /v1/workers/connect`

Tools/models:

- `GET /v1/tools`
- `GET /v1/models`

Memory:

- `POST /v1/memory`
- `GET /v1/memory`
- `DELETE /v1/memory/{id}`

Artifacts:

- `GET /v1/artifacts/{id}`
- `GET /v1/agent-runs/{id}/artifacts`

### Synchronous Chat

`POST /v1/chat` can exist for short hosted chat or read-only help. It must not be used for long-running agent work. Real agent runs are asynchronous through `POST /v1/agent-runs` plus events.

## 7. Wire Protocol

Transport:

- SSE for server-to-client events
- POST for client-to-server messages
- WebSocket only for paired worker connections

### Server-To-Client Events

Required event types:

- `run_started`
- `assistant_delta`
- `assistant_message`
- `tool_call_requested`
- `approval_required`
- `approval_resolved`
- `diff_preview`
- `tool_call_denied`
- `tool_call_result`
- `artifact_created`
- `usage`
- `compaction`
- `run_completed`
- `run_failed`
- `run_cancelled`

`approval_required`, `diff_preview`, and `tool_call_denied` are mandatory because local Vibn already has permission, confirmation, and diff callbacks. Hosted and relayed modes must not lose that safety model.

### Client-To-Server Messages

Messages:

- `user_message`
- `tool_call_result`
- `approval_response`
- `cancel`

### Event Persistence

Every event is written to `run_events` before being streamed. `GET /v1/agent-runs/{id}/events` supports reconnect by `Last-Event-ID` or query cursor.

This makes browser refreshes, worker restarts, client disconnects, and long agent runs survivable.

## 8. Database Schema

Use Neon Postgres as the source of truth. Existing Better Auth tables remain.

Minimum new tables:

```text
api_keys
  id
  user_id
  name
  hashed_secret
  scopes
  last_used_at
  revoked_at
  created_at

worker_pairing_codes
  id
  user_id
  code_hash
  expires_at
  consumed_at
  consumed_worker_token_id
  created_at

worker_tokens
  id
  user_id
  label
  token_hash
  scopes
  concurrency_limit
  last_seen_at
  revoked_at
  revoked_reason
  created_at

projects
  id
  user_id
  name
  kind
  origin
  metadata_json
  created_at

agent_sessions
  id
  user_id
  project_id
  model
  created_at
  last_activity_at

agent_runs
  id
  session_id
  user_id
  project_id
  backend
  status
  model
  worker_token_id
  sandbox_id
  started_at
  ended_at
  error
  cost_cents
  created_at

run_events
  id
  run_id
  seq
  kind
  payload_jsonb
  created_at

tool_calls
  id
  run_id
  seq
  tool_name
  args_jsonb_redacted
  approval_required
  approval_status
  denial_reason
  result_summary
  output_truncated_bytes
  started_at
  ended_at
  ok

artifacts
  id
  run_id
  kind
  mime
  storage_url
  size_bytes
  created_at

memory
  id
  user_id
  project_id
  kind
  body
  source_run_id
  created_at

usage_counters
  id
  user_id
  period_start
  runs_count
  sandbox_seconds
  artifact_bytes
  token_input_count
  token_output_count
```

Indexes:

- `api_keys(hashed_secret)`
- `worker_tokens(token_hash)`
- `agent_runs(status, backend, created_at)`
- `run_events(run_id, seq)`
- `tool_calls(run_id, seq)`
- `artifacts(run_id)`
- `usage_counters(user_id, period_start)`

Queue query for cloud sandbox dispatch:

```sql
SELECT *
FROM agent_runs
WHERE status = 'queued'
  AND backend = 'cloud-sandbox'
ORDER BY created_at
FOR UPDATE SKIP LOCKED
LIMIT 1;
```

## 9. Auth And Tokens

### Auth Types

Use three credential types:

1. Better Auth session cookie
   - dashboard
   - desktop
   - logged-in browser flows

2. `vbn_sk_*` API key
   - SDK consumers
   - server-to-server integrations

3. `vbn_worker_*` paired worker token
   - local worker outbound connection

### Scopes

Minimum scopes:

- `runs:create`
- `runs:read`
- `runs:cancel`
- `sessions:read`
- `sessions:write`
- `projects:read`
- `projects:import`
- `memory:read`
- `memory:write`
- `artifacts:read`
- `worker:connect`
- `worker:execute`
- `tools:read`
- `tools:write`
- `tools:shell`
- `tools:git`
- `tools:mcp`

### Pairing Flow

1. User creates a pairing code in dashboard or CLI.
2. API stores only `hash(code)`.
3. `vibn worker pair <code>` calls the exchange endpoint.
4. API validates code, marks it consumed, creates a worker token.
5. Raw worker token is returned once.
6. Worker stores token locally.
7. Worker uses token for outbound WSS.
8. User can revoke the worker from dashboard.

This mirrors the Gimme Job desktop-token model.

## 10. Railway API Migration

The existing auth app moves to Railway and becomes the consolidated API.

### Deployment Target

- Service: `vibn-api`
- Platform: Railway
- Runtime: Next.js standalone Docker container
- Domain: `api.vibn.dev`
- Database: existing Neon Postgres

### Build/Start

Use a Dockerfile that:

1. installs workspace dependencies
2. generates Prisma client
3. builds Next.js standalone
4. runs `.next/standalone/server.js`

Expected commands:

```text
npx prisma generate
next build
node .next/standalone/server.js
```

### Prisma

Set appropriate binary targets for the Railway Linux image, for example:

```prisma
binaryTargets = ["native", "linux-musl-openssl-3.0.x"]
```

Use the pooled Neon connection string. Railway is long-lived and avoids the worst serverless connection storm pattern, but pooled connections remain the safer default.

### Cutover From `vibn-auth-api.vercel.app`

1. Deploy `apps/api` to Railway at `api.vibn.dev`.
2. Run both deployments against the same Neon database.
3. Smoke test Better Auth sign-in/session on Railway.
4. Change desktop default auth endpoint to `https://api.vibn.dev`.
5. Keep an env/config override for old clients.
6. Convert the Vercel auth deployment to a 308 redirect to `api.vibn.dev`.
7. Keep the redirect for at least 60 days.
8. Decommission the Vercel auth deployment only after logs show no meaningful residual traffic.

## 11. Worker Architecture

### `crates/vibn-server`

Library crate for:

- adapting `vibn-core` into a streaming run driver
- mapping core callbacks to run events
- persisting event records
- sending events to API streams
- driving `ToolHost` implementations

### `crates/vibn-worker`

Binary crate for Railway deployment.

Responsibilities:

- connect to `vibn-api` internal endpoint
- claim queued cloud-sandbox jobs through Postgres or API lease endpoint
- start sandbox workspaces
- hydrate repositories/uploads
- instantiate `Workspace`
- run `vibn-server` driver
- enforce execution policy
- persist artifacts and transcripts
- report completion/failure

### Paired Worker Channel

Endpoint:

- `WSS /v1/workers/connect`

Flow:

1. local worker authenticates with `vbn_worker_*`
2. API marks worker online
3. API dispatches matching runs over the socket
4. worker executes locally
5. worker sends events/tool results back
6. API persists events and fans out to clients

## 12. Sandbox Policy

Cloud sandbox starts conservative.

### Default Allowed Tools

- `read_file`
- `list_directory`
- `search_code`
- `find_files`
- `read_observations` if backed by hosted memory

### Default Preview-Only Tools

- `write_file`
- `edit_file`
- `patch_file`
- selected `git` operations that only inspect state

Preview-only means the worker can produce a diff artifact, but applying it requires explicit approval and policy permission.

### Default Denied Tools

- `run_command`
- mutating `git` operations
- `mcp__*`
- `generate_image`
- `generate_video`
- `install_comfy`
- `download_image_model`
- arbitrary network egress

### Enabling Shell Later

Shell command execution in cloud requires:

- `tools:shell` scope
- command timeout
- output limit
- egress allowlist
- workspace size limit
- audit log
- user/org quota
- approval event for risky commands
- denylist for destructive host-level operations

## 13. Security Requirements

Minimum requirements before public cloud use:

- per-run isolated workspace
- no shared writable filesystem across users/runs
- path containment in all modes
- network egress allowlist in cloud mode
- API key hashing
- worker token hashing
- token revocation
- last-used/last-seen tracking
- scoped authorization
- run timeout
- command timeout
- output truncation
- workspace size limits
- artifact size limits
- concurrent run limits
- durable run event log
- durable tool call audit log
- redacted tool arguments
- redacted command output
- approval events for writes/commands/git mutation
- denied tool call events
- MCP disabled or allowlisted in cloud mode
- signed artifact URLs
- per-user usage counters
- admin kill switch for workers and API keys

Secret redaction should cover:

- environment variable names containing `SECRET`, `TOKEN`, `KEY`, `PASSWORD`, `CREDENTIAL`
- bearer tokens
- common API key patterns
- URLs with token-like query parameters
- provider-specific keys where known

## 14. Cost Plan

Budget target: $200-$300/month at launch.

### Expected Monthly Cost

| Component | Service | Expected range |
|---|---|---:|
| Frontends | Vercel Pro | $20 |
| API container | Railway | $5-$25 |
| Worker container | Railway | $10-$50 |
| Postgres | Neon | $19-$40 |
| Object storage | Cloudflare R2 / S3 / Blob | $0-$10 |
| Redis/cache, if needed | Upstash | $0-$10 |
| Cloud sandbox | Vercel Sandbox | $0-$50 initially |
| Observability | Sentry/logs free tier | $0 |
| LLM tokens | BYOK | $0 to Vibn |
| LLM tokens if managed | Gateway/providers | hard cap at $50-$150 |

BYOK keeps v1 comfortably inside budget. Managed credits require hard caps before public signup.

### Cost Controls

- BYOK default
- per-user concurrent run cap
- per-user daily run cap
- per-user artifact storage cap
- sandbox seconds cap
- no always-on GPU
- no cloud ComfyUI downloads
- no unrestricted free shell execution
- queue depth alerts

### Concurrency Caps

Initial caps:

| Tier | Concurrent runs | Notes |
|---|---:|---|
| Free / BYOK | 1 | serializes free use |
| Hosted paid | 3 | enough for normal active use |
| Paired worker | worker-controlled, default 2 | constrained by local machine |
| Enterprise | custom | later |

Worker sizing at launch:

- one Railway `vibn-worker` container with 1 CPU / 1 GB RAM
- scale manually when queued jobs stay above 3 for more than 60 seconds
- add automated scaling later

## 15. Product And Pricing Stance

Launch pricing should be BYOK-first.

Recommended:

- free or low-cost BYOK tier with strict run/concurrency caps
- paid hosted convenience tier around $10-$20/month
- managed credits only after billing/usage controls exist

Do not launch a managed-credit free tier without hard spend caps.

## 16. Monorepo Layout

Target layout:

```text
vibn/
  apps/
    marketing/          existing, Vercel
    dashboard/          new, Vercel
    api/                existing Better Auth app, moves to Railway,
                        adds /v1/* routes

  crates/
    vibn-core/          refactored runtime
    vibn/               CLI/TUI, local by default, remote optional
    vibn-desktop/       Tauri app, auth endpoint updated
    vibn-server/        new streaming driver library
    vibn-worker/        new Railway worker binary

  packages/
    sdk-ts/             generated OpenAPI client + streaming helpers
    sdk-py/             later generated Python client
```

CLI additions:

- `vibn login`
- `vibn api-key create`
- `vibn run --remote`
- `vibn worker pair <code>`
- `vibn worker start`
- `vibn sessions sync`

Default CLI behavior remains local.

## 17. Implementation Roadmap

### Milestone 0: Groundwork In `vibn-core`

Target: 1 week.

1. Add `ModelBackend`.
2. Implement `OllamaBackend`.
3. Implement `OpenAICompatibleBackend`.
4. Add `Workspace` with local filesystem-backed stores.
5. Add `ToolHost`.
6. Implement `LocalToolHost`.
7. Implement `ReadOnlyToolHost`.
8. Keep CLI/TUI/desktop behavior unchanged.

Exit criteria:

- existing CLI works
- existing TUI works
- existing desktop commands work
- unit tests cover local-backed workspace behavior

### Milestone 1: Railway API Foundation

Target: 1-2 weeks.

1. Dockerize `apps/api` as Next.js standalone.
2. Deploy to Railway.
3. Point `api.vibn.dev` at Railway.
4. Smoke test Better Auth on Railway.
5. Add OpenAPI generation/publishing.
6. Add DB tables for API keys, sessions, runs, events, tool calls, artifacts, worker tokens, usage.
7. Add API-key issuance and validation.
8. Add `POST /v1/sessions`.
9. Add `POST /v1/agent-runs`.
10. Add `GET /v1/agent-runs/{id}/events` with replay cursor.

Exit criteria:

- auth works on Railway
- `/openapi.json` exists
- API key can create read-only run
- events persist and replay

### Milestone 2: Worker Alpha

Target: 1-2 weeks.

1. Add `crates/vibn-server`.
2. Add `crates/vibn-worker`.
3. Deploy worker to Railway.
4. Support queued read-only runs.
5. Persist run events and tool calls.
6. Support cancellation.
7. Add run timeout.
8. Add output limits.

Exit criteria:

- API can enqueue a run
- worker claims and completes it
- event stream shows progress
- reconnect resumes by cursor

### Milestone 3: TypeScript SDK And AppLab Provider

Target: 1 week.

1. Generate `@vibn/sdk` from OpenAPI.
2. Add hand-written streaming wrapper.
3. Add AppLab `VibnExecutionProvider`.
4. Keep current local CLI as priority 1.
5. Add cloud fallback as priority 3.
6. Expose actual backend used in AppLab responses.
7. Preserve AppLab path containment and output truncation.

Exit criteria:

- AppLab with local Vibn behaves like today
- AppLab without local Vibn can fall back to hosted read-only cloud execution
- UI or response metadata identifies local vs cloud

### Milestone 4: Paired Local Worker

Target: 1-2 weeks.

1. Add pairing-code endpoints.
2. Add `vibn worker pair`.
3. Add `vibn worker start`.
4. Add worker WSS connection.
5. Dispatch runs to paired worker.
6. Add local approval event flow.
7. Add worker revocation UI/API.

Exit criteria:

- paired worker can execute a run on local project
- API streams events to client
- revoked worker cannot reconnect
- AppLab can use paired worker when local CLI is unavailable

### Milestone 5: Cloud Sandbox

Target: 2 weeks.

1. Integrate Vercel Sandbox.
2. Add cloud workspace hydration from git URL.
3. Add upload/archive import.
4. Implement `SandboxToolHost`.
5. Enforce denylist and egress policy.
6. Produce diff artifacts.
7. Support explicit approval to apply safe writes.

Exit criteria:

- cloud run can analyze a public repo
- cloud run can produce a diff preview
- shell/MCP/ComfyUI are denied by default
- audit records exist for every tool call

### Milestone 6: Dashboard And Billing Foundation

Target: 1-2 weeks.

1. Add `apps/dashboard`.
2. Add API key management.
3. Add worker management.
4. Add usage view.
5. Add Stripe subscription shell.
6. Add run quota enforcement.

Exit criteria:

- user can manage keys/workers
- user can view usage
- quotas prevent budget abuse

### Milestone 7: Public SDKs And Docs

Target: 1 week.

1. Publish `@vibn/sdk`.
2. Add API docs from OpenAPI.
3. Add examples for local-first AppLab, paired worker, and cloud sandbox.
4. Add Python SDK if demand exists.

### Milestone 8: Managed Credits, GPU, And Advanced Cloud

Later.

1. Add managed LLM credits with hard caps.
2. Add on-demand GPU/image/video support.
3. Add optional ComfyUI remote endpoint integration.
4. Add regional workers.
5. Add enterprise/self-host deployment story.

## 18. Testing And Verification

### Core Tests

- `ModelBackend` trait compatibility
- local Ollama backend unchanged
- OpenAI-compatible backend request/response mapping
- local `Workspace` paths match current `~/.vibn` behavior
- hosted `Workspace` stores write expected records
- `ToolHost` policies deny/allow correctly
- `ReadOnlyToolHost` blocks writes, commands, git mutation, MCP

### API Tests

- Better Auth session still works after Railway move
- API key creation returns raw secret once
- API key hash validation works
- revoked API key fails
- run creation enforces scopes
- event stream replays by cursor
- cancellation changes run state
- worker token pairing/exchange/revocation

### AppLab Tests

- local provider selected when Vibn exists
- cloud provider selected only when local unavailable
- project path outside `PROJECTS_BASE` is rejected
- execution mode returned in response
- cloud fallback does not request arbitrary local host paths

### Worker Tests

- worker claims queued run
- worker respects timeout
- worker truncates output
- worker persists run events
- worker persists tool calls
- worker handles cancellation
- paired worker reconnect updates `last_seen_at`

### Security Tests

- shell denied by default in cloud
- MCP denied by default in cloud
- `install_comfy` denied in cloud
- `download_image_model` denied in cloud
- secrets redacted in args/output
- artifact access requires auth
- revoked worker cannot receive jobs

## 19. Operational Runbooks

### Deploy API

1. Build Docker image for `apps/api`.
2. Run Prisma generate.
3. Deploy Railway service.
4. Run schema migration/push.
5. Check `/health`.
6. Check Better Auth session route.
7. Check `/openapi.json`.

### Deploy Worker

1. Build `vibn-worker` Docker image.
2. Deploy Railway service with internal API URL.
3. Check worker heartbeat.
4. Enqueue test run.
5. Verify event stream.

### Kill Switches

Required operational controls:

- disable cloud sandbox globally
- disable shell tools globally
- disable MCP globally
- revoke API key
- revoke worker token
- cancel run
- mark worker offline
- set per-user run cap to zero

## 20. Risks And Mitigations

### Risk: Duplicating Auth

Mitigation:

- reuse `apps/api` Better Auth
- move deployment target, not auth model
- add API keys and worker tokens alongside existing user table

### Risk: Cloud Costs Exceed Budget

Mitigation:

- BYOK at launch
- hard concurrency limits
- hard sandbox-second caps
- artifact storage caps
- no always-on GPU
- no managed credits until billing exists

### Risk: Cloud Tool Execution Is Unsafe

Mitigation:

- read-only first
- deny shell/MCP/ComfyUI by default
- audit every tool call
- require approval events
- egress allowlist
- isolated workspace

### Risk: `vibn-core` Refactor Breaks Local Product

Mitigation:

- local `Workspace` mirrors current `~/.vibn`
- `LocalToolHost` mirrors current behavior
- CLI/TUI/desktop tests before API work
- no behavior change in Milestone 0

### Risk: AppLab Unexpectedly Uses Cloud

Mitigation:

- provider order is explicit
- local availability cached and surfaced
- response includes backend used
- cloud fallback can be disabled by config

### Risk: Long Runs Disconnect

Mitigation:

- persist events before streaming
- support cursor replay
- run state stored in Postgres
- cancellation endpoint
- paired worker reconnect protocol

## 21. Non-Goals For V1

- always-on hosted GPU
- unrestricted public shell execution
- unrestricted hosted MCP
- hosted ComfyUI installation/download
- browser/desktop automation in cloud workers
- multi-user collaborative sessions
- enterprise private deployments
- fine-tuning-as-a-service
- cloud replacement for local Vibn

## 22. Final Position

Vibn should become a hosted API product without giving up its local-first trust model.

The first successful cloud migration is not a fully hosted coding agent replacing local execution. It is:

- a consolidated Railway API that reuses existing Better Auth
- a separate Rust worker for long-running agent execution
- OpenAPI and SDKs
- local-first AppLab provider selection
- paired local workers for private local projects
- cloud sandbox fallback for unavailable local execution, public repos, demos, and explicit cloud runs
- durable run events and tool-call audit logs from the beginning
- BYOK-first hosted model calls through Vercel AI Gateway

This plan keeps the launch architecture realistic under $200-$300/month, preserves AppLab's current local behavior, and creates a credible path from local Vibn to a public API, SDKs, dashboard, and cloud workspaces.
