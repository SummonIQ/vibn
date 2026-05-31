# Vibn — Business Plan

> Status: draft v2. This is a living doc. Update as we ship.

## TL;DR — what we're building and why

Vibn is **the local-first AI coding agent you actually own**. Free desktop app, open-core, capable out of the box. Lives wherever you already work — your IDE, your terminal, your git hooks, a global hotkey. A curated marketplace of skills, MCP servers, models, rule packs, and recipes extends it to anything you need. Pro tier adds cloud-shape conveniences (sync, hosted background agents, premium skills). Team tier adds governance, shared context, and audit logs.

**The wedge for users:** "I want a real agent that respects my data, lives in my workflow, and doesn't lock me into one vendor's models or one company's IDE."

**The wedge for us:** the free app drives distribution; the marketplace and team layer are the business. We're not in the inference business — we never sit in the token-resale path — so we don't compete with our users' API providers, don't have a gateway to operate, and don't ask anyone to sign up just to use the product.

This document covers positioning, pricing, the open-source plan, the marketplace shape, integration surfaces, vertical opportunities, a feature backlog ranked by ship-time, and the first 90 days.

---

## Positioning

### One-liner

> **The local-first AI coding agent. Free, open, and in every part of your workflow.**

### What this is NOT

- Not another Cursor — we don't replace your editor; we live alongside it.
- Not ChatGPT-but-local — we're an agent: tools, MCP, file edits, permissioned actions.
- Not Ollama UI — Ollama is one backend; you can also bring your own API keys.
- Not a cloud coding tool — cloud is opt-in per conversation, never the default.
- Not a metered subscription — no credits, no token resale, no usage anxiety.

### Why open source from day 1, not "after stars"

1. The privacy claim is the wedge. "Your code never leaves your machine" is only believable if anyone can audit it.
2. OSS is our distribution. We can't outspend incumbents; we can outflank them on trust and openness.
3. OSS forces clean architecture. The paid layers (marketplace backend, sync, team) must stand alone — exactly what we want anyway.
4. The paid product can't depend on hiding the agent. If our value is "we run the agent and you can't see it," we lose to anyone who runs theirs cheaper.

**License:** Apache 2.0 for the open core (`crates/vibn-core`, `crates/vibn`). The desktop bundle, marketplace backend, cloud sync, and team backend stay private and commercial. HashiCorp / GitLab / Sentry / Linear pattern.

---

## Product offering

Three layers, distinct revenue, deeply integrated:

### 1. The Desktop App (free, open core)

Free signed bundled download. Tauri-based. On install:

- Ships with `vibn-core` + `vibn-desktop` (open source). Audit-friendly.
- Pre-configured with a curated list of free local models, one-click install via Ollama.
- Built-in **marketplace** for skills / MCP servers / models / rule packs / recipes / themes / bundles.
- Bring your own API keys for hosted models (Claude, GPT, Gemini, etc.) — we never proxy them. The pass-through is direct; we add observability and routing if you want.
- Default config emphasizes local; hosted models are explicit per conversation.

This is the on-ramp. Most users will never pay us. That's by design — the free experience makes the privacy claim credible.

### 2. The Marketplace (revenue)

Curated, signed, sandboxed. Free + paid items. We take 20% of paid items; 80% to creators (more creator-friendly than the 30% Apple/Google standard).

Six SKU types — covered in detail below. The marketplace is **the** economic engine.

### 3. Pro & Team (subscriptions)

Pro is for individuals who want cloud-shape conveniences without the cloud lock-in. Team is for engineering organizations who need governance.

---

## Pricing

| Tier | $/mo | Who | What |
|---|---:|---|---|
| **Free** | $0 | Everyone | Desktop app, all local models, MCP support, observations, hooks, transcripts, free marketplace items, BYO API keys for hosted models |
| **Pro** | $8 | Solo devs who want polish + cloud convenience | Cloud sync of skills/observations/config across machines, premium skills/MCPs we make, hosted background agents, themes, advanced cost analytics for your own API usage, priority marketplace |
| **Team** | $15/seat | Engineering teams | Pro per-seat + shared observations + shared skills + audit log + SSO + admin + per-project budgets + shared marketplace pool |
| **Enterprise** | custom | Regulated industries | Team + on-prem cloud sync + SOC2 + custom SLA + private marketplace |
| **Marketplace items** | one-off or sub | Anyone | Pay per item; we take 20% |

No metered usage. No credit balances. No "you used 73% of your allowance" emails.

---

## The marketplace — six SKU types

This is where Vibn becomes more than "another agent." Each SKU is signed, versioned, sandboxed, and one-click installable from the desktop app.

### 1. Skills

Agentic workflows. A skill is: a system prompt + allowed tools + recommended model + example invocations + manifest. Examples:

- "PR Reviewer Pro" — runs against a diff, posts inline comments
- "React 18 → 19 Migrator" — multi-file refactor with verification
- "Stripe Integration Auditor" — checks for common mistakes
- "Postgres Query Optimizer" — runs `EXPLAIN`, proposes improvements
- "Code Tour Guide" — onboarding agent that walks a new dev through a codebase

Authors keep 80%. Free skills are the norm; premium skills run $5-50 one-off or $2-5/mo.

### 2. MCP Servers

Curated registry of Model Context Protocol servers, one-click install. Categories: source control, project management, payments, observability, databases, AI services, custom.

Three flavors:
- **OSS** — community servers, free.
- **Premium self-host** — paid one-off, runs on your machine.
- **Hosted** — we run it for you. Subscription pricing. Solves the "I don't want to manage tokens / Docker / cron for this server" problem.

### 3. Models

Fine-tuned local models distributed through the marketplace. Buy once, download, run forever via Ollama.

- "vibn-react-coder-7b" — fine-tuned on React-shaped tasks
- "vibn-rust-7b" — Rust idioms
- "vibn-sql-3b" — fast SQL completion
- "vibn-prose-13b" — for writers/researchers, not coders

Premium tier gets monthly retrained versions on opt-in telemetry. Free tier gets the base model forever.

### 4. Rule Packs

Opinionated `OBSERVATIONS.md` files. The Cursor-rules pattern but signed, versioned, and authored by people you can trust:

- "Next.js 16 conventions"
- "Production-grade Python"
- "Stripe payment best practices"
- "HIPAA-compliant code patterns"

These are essentially "expert advice packaged as agent context." Cheap to author, high value, easy to keep current.

### 5. Recipes

Multi-step playbooks — composed agentic workflows that wire together skills, MCPs, and rule packs. Examples:

- "Set up a new SaaS: Next + Prisma + Stripe + Vercel + Resend"
- "Migrate this codebase from REST to tRPC"
- "Stand up an LLM eval pipeline"

Recipes are how non-experts get expert-quality output. They're also a marketplace runaway-success candidate because they bundle other paid items.

### 6. Themes

UI customization. Low-margin, high-affection. Raycast proved this is real.

### Bonus: Bundles

A bundle is a manifest pointing at N skills + N MCPs + N rule packs + N recipes — a curated package installable as a unit. Bundles enable the **vertical strategy** (see below): "Vibn Legal", "Vibn Author", "Vibn DevOps" are bundles, not separate apps. Same desktop binary, different installed loadout.

This single primitive lets us enter any vertical without forking the codebase.

---

## Where Vibn lives in your workflow

The desktop app is the home base, but a real workflow tool reaches you wherever you are. Ranked by leverage:

### Tier 1 — ship-fast, immediate daily-use uplift

1. **IDE extension** (VS Code first, then Zed, JetBrains) — covered in detail in the next section. This is the single biggest leverage point.
2. **Global hotkey + tray** — `⌘⇧Space` opens Vibn from anywhere. Makes the app ambient instead of an app-you-launch.
3. **CLI as pipe-friendly** — `git diff | vibn "write a commit message"`. Already mostly there; lean into stdin/stdout for one-shot piping.
4. **Git hooks** — pre-commit Vibn runs review on staged diff; pre-push runs a deeper check. One-line installer.

### Tier 2 — bigger lifts, bigger payoff

5. **GitHub bot** — `@vibn review this PR` posts an inline-comment review. Hosted (Pro feature) or self-hosted (free). PR review is the highest-ROI dev workflow we can own.
6. **Linear / Jira / Asana triggers** — new ticket → Vibn drafts an implementation plan + attaches it. Mostly skill packaging on top of MCP servers.
7. **Slack / Discord / Teams bot** — chat with Vibn from team chat; agent runs on a connected user's machine. Team-plan accelerator.
8. **Browser extension** — right-click → Ask Vibn. Captures Stack Overflow / docs / GitHub PR context. Useful but later than IDE.
9. **Mobile companion** — read-only iOS/Android app. Monitor background agents, approve permissioned actions remotely.

### Tier 3 — universal workflow patterns

These cut across every developer's day, regardless of stack:

- **Standup summarizer** — what shipped yesterday, what's in flight, what's blocked
- **On-call assistant** — paged → Vibn pulls logs, runbooks, recent deploys, drafts incident notes
- **PR triage** — daily run that summarizes every PR in your inbox by urgency + impact
- **CI failure investigator** — webhook on failure → agent investigates → posts diagnosis + proposed fix
- **Meeting prep** — pre-meeting brief from codebase + Linear + Slack context
- **Inbox processor** — GitHub notifications, Linear, PR review requests bundled into one daily run

Each is a Skill in the marketplace. The platform integrations (Slack, GitHub, Linear) come from MCP servers. The orchestration is Vibn.

---

## The IDE extension — what it does and why it matters

The single highest-leverage integration we'll ship. A separate section because it's the difference between "Vibn is an app you sometimes open" and "Vibn is always there."

### What it actually does

**Context, surfaced automatically**
- When you trigger Vibn (selection, hotkey, command palette), the agent receives: the selected code, the file path, language, cursor position, git branch, your uncommitted diff, your open-file list, and the active LSP diagnostics.
- No more "let me explain what I'm looking at" — the agent already knows.

**Inline interactions**
- **Quick chat** — a small popup at the cursor for one-shot questions. Answer renders inline; "send to desktop app" promotes the conversation to a full session.
- **Ghost edits** — agent proposes multi-line changes; accept with Tab, reject with Esc. Cursor's pattern, but agent-driven so it can edit across files (with permission).
- **Code lenses** — "Ask Vibn about this function" appears above every function. Click → opens a focused skill conversation pre-loaded with the function and its call sites.
- **Quick fixes** — when a linter or LSP flags an error, "Vibn: fix" joins the lightbulb menu.

**Skill invocation from the editor**
- Right-click a file or selection → "Run skill: ..." with installed skills shown.
- Command palette: `Vibn: Run Skill` for keyboard-driven invocation.
- Skill output goes wherever it makes sense — inline edit, side panel, new file, or back to the desktop app for richer interaction.

**Diff-aware operations** (one keypress each)
- `Vibn: Explain my changes` — runs against staged + unstaged
- `Vibn: Write commit message` — uses git diff, produces a Conventional Commits-style message
- `Vibn: Review my diff before push` — runs the team's PR-review skill on your local diff before you even push
- `Vibn: What did I just break?` — semantic diff analysis with risk callouts

**Bidirectional sync with the desktop app**
- Edits made in the desktop app appear instantly in your editor (file watcher; conflict-safe).
- Editor selection changes get pushed to the desktop app's active context.
- Hotkey to "promote this to the desktop app" — opens with the current editor state pre-loaded.

**Status, ambient**
- Status-bar item showing: current Vibn model, active skill, session token count (when using BYO keys).
- Notification badge when a background agent completes a task on this project.

### The value, from the developer's side

- **Speed.** No alt-tab. The agent is one keystroke away with full context already loaded. Quick questions resolve in 2 seconds instead of 20.
- **Trust.** Edits feel native — they're in your editor, with your themes, your undo stack, your git workflow. No "agent did something in a chat window, now I have to paste it back."
- **Continuity.** A question that starts as inline chat can promote to the desktop app for deeper interaction without losing the conversation.
- **Skill access everywhere.** Marketplace skills are reachable from the editor command palette. Discovering a skill becomes a 2-second action, not a "find the app, open it, navigate, install" process.
- **Diff-aware as default.** Most "ask the AI" questions are really "ask the AI about what I just changed." Making that one keystroke is huge.

### The value, from the business's side

- **Stickiness.** An installed editor extension is harder to uninstall than a desktop app. People keep their VS Code setup for years.
- **Daily-use.** Most desktop apps get opened once a day. Editors are open 8 hours a day. The extension turns daily-use from "session" into "continuous."
- **Differentiator vs. Cursor.** Cursor wants to be your editor. We say: keep VS Code / Zed / JetBrains with all your themes, extensions, keybindings, settings — Vibn just augments it. That's a much easier "yes."
- **Differentiator vs. Copilot.** Copilot is completion + chat. No agent depth, no marketplace, no local. We're an actual agent with tools, file edits, MCPs, and a marketplace — all summonable from inside the editor.
- **Differentiator vs. Continue.dev.** Continue is the closest competitor. Differentiation comes from: the desktop app companion (deeper conversations live there), the marketplace (Continue has none), our local-first/OSS positioning, and the cleaner skill packaging.
- **Bridge to the marketplace.** A skill marketplace is interesting *only* if skills are discoverable and reachable in the flow. The extension makes the marketplace part of the daily editing experience.

### Editor priority

1. **VS Code** — biggest audience by far. Ship first. The extension API is mature; most of our work goes here.
2. **Zed** — small but vocal audience, fast-growing, friendly to extensions. Ship second.
3. **JetBrains** (IntelliJ family) — large audience, painful extension API. Wait until VS Code is stable.
4. **Neovim** — small but devoted; LSP-friendly architecture means a thin Lua plugin can reuse most of our work. Community PR potential.
5. **Cursor** — yes, even Cursor. Their users still want our marketplace + local-first. Same VS Code extension works.

### MVP scope (4-week build)

Strip down to the loadable surface:

- VS Code extension that connects to the desktop app via a local socket.
- Three commands: `Vibn: Ask`, `Vibn: Run Skill`, `Vibn: Explain my diff`.
- One inline interaction: quick-chat popup at cursor with the selection as context.
- Status bar item showing active model.
- "Promote to desktop app" command.

Everything else (ghost edits, code lenses, quick fixes, JetBrains, Zed) ships in v2.

---

## Beyond developers — vertical opportunities

The architecture (local LLM + filesystem + shell + MCP + per-action permissions) is more general than coding. The wedge — local-first, audit-friendly, owned — sells *harder* in professions where cloud AI is a compliance problem.

We don't fork the product per vertical. We ship **bundles**: skill packs + MCP servers + rule packs + recommended models, installable as one item. "Vibn Legal" is a $39 one-time marketplace purchase that installs everything a lawyer needs and configures defaults appropriately.

Verticals ranked by fit:

### Legal (lawyers, paralegals, in-house counsel)

**Why local-first wins:** Privilege + confidentiality. Cloud AI is a malpractice risk in most firms. Many state bars have issued opinions discouraging or forbidding it for client matters. Local-first sidesteps the entire compliance conversation.

**What we'd ship:** Contract analysis skills, case research playbooks, doc review workflows, drafting skills (motions, letters, memos). MCP servers for legal research databases (Westlaw, LexisNexis — both have public APIs). Possibly fine-tuned legal models in the marketplace.

**Channel:** Bar associations, legal-tech conferences, ABA tech survey audience. Probably needs a partnership with a legal-tech consultancy for credibility.

### Healthcare (therapists, clinicians, small practices)

**Why local-first wins:** HIPAA mandates local processing for PHI in nearly all cases. Cloud AI is functionally illegal for most clinical workflows without a BAA. Our market here is whoever can't or won't get a BAA with OpenAI/Anthropic — which is the vast majority of small practices.

**What we'd ship:** Clinical note assistance, treatment plan drafts, intake summarization, billing-code suggestion. MCP servers for EHRs that expose APIs (Epic via their MCP-like APIs, Athena, etc.). "PHI-safe mode" branding that makes the local guarantee explicit.

**Channel:** Direct to small practices via clinician communities. The market here is large and dramatically underserved.

### Accounting / Tax / Bookkeeping

**Why local-first wins:** Client financial data. NDA-shaped work. Sole proprietors often handle multiple clients with conflicting confidentiality requirements.

**What we'd ship:** QuickBooks / Xero MCP servers, Excel/PDF-aware skills, reconciliation workflows, audit prep, tax-research skills. A "client context isolation" feature (in-progress concept) that keeps separate client matters from contaminating each other.

**Channel:** Tax-prep communities, small-CPA forums.

### Research / Academia

**Why local-first wins:** Unpublished work, embargoed data, peer-review confidentiality. Researchers actively worry about cloud AI training on their pre-publication work.

**What we'd ship:** PDF library search (point Vibn at your downloads folder), citation management (Zotero MCP), LaTeX skills, peer-review assistant skills, literature-review workflows.

**Channel:** Academic Twitter/Mastodon, Open Science communities, university IT departments looking for privacy-respecting AI options.

### Writers / Authors / Journalists

**Why local-first wins:** Drafts, sources, embargoed stories, unpublished manuscripts. Same trust gap as researchers.

**What we'd ship:** Manuscript-aware skills (long-context editing), fact-checking workflows, source-management, style-consistency, character/plot trackers (for fiction). Possibly fine-tuned prose models in the marketplace.

**Channel:** Indie-author communities, journalism forums.

### Consultants / Solo professionals

**Why local-first wins:** NDA'd client work, multi-engagement context isolation. The same person is bound by five conflicting confidentiality agreements simultaneously.

**What we'd ship:** Per-client context boundaries, proposal generation skills, research synthesis, document drafting. Per-client "vaults" in the desktop app.

**Channel:** Solopreneur communities, consulting forums.

### Sysadmins / DevOps / SRE

**Why local-first wins:** Adjacent to devs but with a sharper compliance lens — these people handle production credentials and incident data that absolutely cannot leave the org.

**What we'd ship:** Runbook automation skills, log analysis workflows, incident-response playbooks, Terraform/Pulumi skills, cloud-console MCPs.

**Channel:** SRE/devops forums; this audience overlaps significantly with our dev launch audience.

### Where to bet first

For the first 6 months, **dev-first is right** — devs evangelize, contribute marketplace items, and tolerate rough edges. But two moves now lay groundwork:

1. **Make the bundle a first-class SKU type from day 1.** Bundles enable vertical strategy without code forks.
2. **Pick ONE non-dev vertical for a proof-point by month 6.** I'd argue **legal** is the strongest because the compliance wedge is sharpest, but **writing/research** is the easier path because we can author "Vibn Author" ourselves without needing legal-tech partnerships.

---

## Innovative features — fast-shippable

Categorized by **time-to-ship**.

### Tier 1 — Days (ship in 1–2 weeks each)

1. **One-click model installer** — curated local models, hardware-aware recommendations, one-tap Ollama integration. The free experience becomes "open Vibn, pick a model, go."
2. **Receipts** — every assistant message shows what was called, what tokens were used, what it would have cost (for BYO-key users). Click to drill in. Trust through transparency.
3. **OSS landing repo + docs site** — split `crates/vibn-core` + `crates/vibn` to a public repo, Apache 2.0. Static docs site at `vibn.dev/docs`. Restore the public GitHub link.
4. **Global hotkey + tray icon** — `⌘⇧Space` opens Vibn anywhere.
5. **CLI piping** — `git diff | vibn "write a commit message"`. Stdin/stdout-friendly mode for one-shot agentic tasks.
6. **Real-time cost meter** — for BYO-key users, show the cost of every message in real time. The "wait, that one prompt cost me $3?" moment changes habits.

### Tier 2 — Weeks (each takes ~3-4 weeks)

7. **Skill Marketplace v1** — author signing, sandboxing, payments via Stripe Connect. Launch with 30 free + 10 paid skills, all built by us.
8. **MCP Marketplace v1** — curated registry of 20-30 servers with one-click install.
9. **IDE Extension (VS Code)** — MVP scope per the section above.
10. **Git hooks one-line installer** — pre-commit / pre-push hooks that run a Vibn skill.
11. **Rule Pack Marketplace** — 10-15 launch rule packs by us, plus author-submission flow.
12. **Recipe Marketplace v1** — launch with 5-10 multi-step playbooks.
13. **Diff-aware mode** — Vibn detects uncommitted changes; new built-in commands operate on the diff.

### Tier 3 — Quarter (1-3 months)

14. **Cloud sync** (Pro tier headline feature) — skills, observations, config sync across machines. Conflict-safe.
15. **Team observations sync** — shared `OBSERVATIONS.md` per org, new teammates inherit context automatically.
16. **Audit log + replay** — every tool call logged (for teams). Replay re-runs a session deterministically.
17. **Background agents (local)** — daemon that runs scheduled Vibn tasks. "Every morning, scan PRs and summarize."
18. **GitHub bot** — `@vibn review this PR` runs a skill, posts inline comments.
19. **Bundles** — first-class SKU. Launch with "Vibn Author" or "Vibn DevOps" as the proof-point.
20. **Linear / Jira / Slack triggers** — webhook → background skill run → notification.

### Tier 4 — Months (Q2-Q3 and beyond)

21. **Vibn-specialized fine-tunes** — small models trained specifically on agentic tool-call patterns. Free local tier; Pro gets monthly retrained versions.
22. **Vertical bundle pilot** — "Vibn Legal" or "Vibn Healthcare". Requires partnership with domain experts.
23. **Mobile companion** — read-only iOS/Android, approve permissioned actions remotely.
24. **Voice mode** — local Whisper + TTS. Speak prompts; agent narrates.
25. **Branchable conversations** — fork at any transcript point; run branches in parallel.
26. **BYO Cluster** — expose a remote machine's Ollama as a local-feeling model backend.
27. **JetBrains + Zed extensions**.
28. **Workflow recorder** — watch the user perform a task once; extract a re-runnable skill.
29. **Org-config-as-code** — your team's Vibn config IS a Git repo. PR review for prompt changes.
30. **On-prem / self-host for Enterprise**.

---

## The first 90 days

### Days 0–14 — the OSS launch

- **Day 0–3:** Pick license (Apache 2.0). Audit `crates/vibn-core` + `crates/vibn` for secrets / customer data / embarrassing TODOs. Carve to a clean public repo. Author CONTRIBUTING / CODE_OF_CONDUCT / architecture doc.
- **Day 3–7:** Docs site at `vibn.dev/docs`. Quickstart that gets `cargo install vibn` → working agent in 5 minutes. Restore public GitHub link on marketing.
- **Day 7–12:** Build one-click model installer + receipts + global hotkey + tray icon.
- **Day 12–14:** Soft launch. HN ("Show HN: Vibn — local-first AI coding agent, Rust + Tauri, open source"). r/LocalLLaMA. Lobsters. Discord opened. Tweet thread.

**Goal at end of week 2:** 500 stars, 100 desktop downloads, opt-in Discord with 50 members.

### Days 15–45 — the marketplace foundation

- **Days 15–25:** Marketplace backend MVP (auth via better-auth in `apps/api`, item registry, signing, Stripe Connect for payouts).
- **Days 20–30:** Skill marketplace v1 launches with 30 skills by us.
- **Days 25–35:** MCP marketplace v1 with 20-30 servers.
- **Days 35–40:** Rule pack marketplace + 10 launch rule packs.
- **Days 40–45:** CLI piping. Git hooks one-line installer.

**Goal at end of week 6:** 2,000 stars, 1,000 downloads, first $500/mo in marketplace GMV.

### Days 46–90 — the IDE extension + Pro launch

- **Days 46–70:** VS Code extension MVP (Vibn: Ask / Run Skill / Explain Diff + status bar).
- **Days 60–75:** Cloud sync v1 (Pro feature). Stripe subscription SKUs.
- **Days 75–85:** Pro tier launches. First paid Pro users.
- **Days 85–90:** Team backend MVP (shared observations + audit log v1). Reach out to ~10 small engineering teams for Team beta.

**Goal at end of week 13:** 5,000 stars, 5,000 downloads, 200 Pro subscribers ($1,600 MRR), $1,500/mo marketplace GMV, 5 Team beta orgs.

### Days 91–180 — the bundle proof + vertical

- VS Code extension v2 (ghost edits, code lenses).
- Zed extension MVP.
- Background agents (Pro feature).
- Bundles as first-class SKU.
- Pilot vertical bundle ("Vibn Author" or "Vibn DevOps") — proves the cross-vertical strategy.
- First Team plan customers convert from beta.

**Goal at end of month 6:** 15,000 stars, 20,000 downloads, $15k MRR (subs + marketplace + Team).

---

## How we run the business

### The team

- You (Steven) + me (Claude as collaborator). Solo founder for accounting purposes.
- Hire #1 only after $20k MRR. Probably someone who can own the marketplace backend + Stripe + creator-support while you focus on the agent + desktop + extensions.

### Infrastructure

- **Marketing site:** Vercel (already shipped).
- **Docs:** Vercel sub-route.
- **Marketplace backend + auth + sync:** `apps/api` (better-auth + Prisma + Neon, already scaffolded).
- **Stripe + Stripe Connect** for subscriptions and creator payouts.
- **Telemetry:** opt-in PostHog from the desktop app. Server metrics via OpenTelemetry.

### What we charge ourselves to avoid

- **Don't open-source the marketplace backend, sync, or team backend.** Open core means the *agent* is open. The platform is closed.
- **Don't ever degrade the free experience to push Pro.** If free becomes less capable to nudge upgrades, we lose distribution.
- **Don't ship features that pull users toward the cloud against their will.** Cloud is opt-in per conversation, period.
- **Don't take VC until $50k MRR or a strategic that genuinely gets local-first.** This shape can be profitable as a small team for a long time. Optionality matters.
- **Don't over-curate the marketplace to the point of stalling.** First 100 items, we author. After that, lean toward "approve unless concerning."

### What we charge ourselves to do

- **Ship a public changelog every week.** Even one bullet point. Visible momentum is the OSS heartbeat.
- **Reply to every GitHub issue within 48 hours.** Acknowledgment, not necessarily a fix.
- **Open the Discord on day 0.** Community is the product.
- **Eat our own dogfood.** Use Vibn to write Vibn. Every dogfood-friction issue is P0.

---

## Risks

| Risk | Likelihood | Mitigation |
|------|-----------:|------------|
| Cursor / Continue ship a competing marketplace | High | We have local-first + open-source positioning + multi-editor support. They have IDE lock-in but limited marketplace appetite. |
| Ollama gets acquired or pivots | Medium | Maintain a thin abstraction. Eventually ship our own minimal runtime. |
| Marketplace cold-start (not enough authors) | High | Author the first 100 items ourselves. Aggressive creator-friendly rev share (80/20). |
| Single skill/MCP contains malware | Medium | Curate aggressively. Sandbox by default. Signing keys. Trust score per author. |
| Provider APIs (Anthropic etc.) restrict BYO-key use | Low | We never proxy; users hold their own keys. No relationship for us to lose. |
| Audit log gets us regulated earlier than expected | Medium | SOC2-aware logging on day 1 even if uncertified. |
| Verticals require domain expertise we don't have | High | Partner with practitioners per vertical. Don't ship "Vibn Legal" without a lawyer's name on it. |

---

## Decisions needed

1. **License:** Apache 2.0 for the open core. OK? (Alternatives: MIT, BSL.)
2. **Public repo split:** `SummonIQ/vibn-core` + `SummonIQ/vibn-cli` separate, or one combined public `SummonIQ/vibn-oss`?
3. **First vertical bundle:** "Vibn Author" (easier, we can dogfood) or "Vibn DevOps" (closer to dev audience, less external expertise needed)?
4. **Creator rev share:** 80/20 (creator-friendly) or 70/30 (Apple/Google standard)?
5. **Marketplace currency:** USD only at launch, or multi-currency from day 1 via Stripe?
6. **VS Code extension distribution:** marketplace-only, or also direct VSIX download for users who don't want MS account?

---

## Appendix A — Competitive scan

- **Cursor** — closed IDE, hosted, ~$20/mo. Brilliant UX. No local story, weak marketplace appetite, full IDE replacement. Our differentiation: live alongside any editor, local-first, open core, deep marketplace.
- **Continue.dev** — IDE plugin, OSS, mostly hosted. Strong local story, weaker agent depth, no marketplace. Our differentiation: standalone desktop companion, marketplace, deeper skills.
- **Aider** — CLI, OSS, hosted-bias. Strong reputation, cult following. Our differentiation: GUI option, marketplace, multi-surface (CLI/desktop/IDE).
- **Claude Code** — CLI, hosted-only. Strong agent depth. Our differentiation: any model, local, open, marketplace, multi-vertical.
- **Copilot** — IDE extension. Completion + chat. No agent depth, no marketplace, no local. Our differentiation: real agent with tools, file edits, MCPs.
- **Raycast** — productivity launcher. Our differentiation: we're the agentic-coding-specific version of their model, applied to coding (and beyond).
- **Ollama** — local runtime. We sit on top. Relationship rather than competition.

Nobody is doing exactly this combination. The closest is "Cursor + a marketplace + open-core" — which is everything Cursor *isn't*.

---

## Appendix B — Why the marketplace bundle is the right move

- **Bundles enable verticals without forking.** "Vibn Legal" is a manifest, not a product line. Same binary serves twelve verticals.
- **Creator economy compounds.** Every author who ships a popular skill becomes a marketing surface. We don't have to evangelize alone.
- **Switching cost is configuration cost.** A user's installed skills, MCP servers, and observations become their environment. Leaving means rebuilding — a real moat without lock-in tactics.
- **Marketplace revenue is predictable on a 6-12 month horizon.** Subscription MRR for Pro/Team is predictable monthly. One-off marketplace purchases smooth out via Stripe Connect's payout cadence.
- **It's not a token-resale business.** We don't compete with our users' API providers. We don't have inference costs. Our COGS is hosting + Stripe fees + creator payouts — clean economics.

---

_Last updated: 2026-05-26. Next review: weekly until launch._
