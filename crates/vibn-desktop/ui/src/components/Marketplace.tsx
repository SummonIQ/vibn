import { useEffect, useState, useMemo } from "react";
import { Button } from "./ui/button";
import { McpIcon, SkillIcon } from "./McpIcon";
import { api } from "../api";
import { cn } from "../lib/utils";

type Tab = "mcp" | "skills";

interface McpListing {
  name: string;
  slug: string;
  command: string;
  args: readonly string[];
  description: string;
  homepage?: string;
  env_note?: string;
  vendor?: string;
  category: "code" | "search" | "data" | "comms" | "design" | "ops" | "ai" | "general";
}

const MCP_LISTINGS: McpListing[] = [
  // Official reference
  { name: "Filesystem", slug: "filesystem", command: "npx", args: ["-y", "@modelcontextprotocol/server-filesystem", "$HOME"], description: "Read, write, and search files in allowed local directories.", homepage: "https://github.com/modelcontextprotocol/servers/tree/main/src/filesystem", vendor: "modelcontextprotocol", category: "code" },
  { name: "Git", slug: "git", command: "uvx", args: ["mcp-server-git"], description: "Inspect repository state, diffs, log, branches, and commits.", homepage: "https://github.com/modelcontextprotocol/servers/tree/main/src/git", vendor: "modelcontextprotocol", category: "code" },
  { name: "Fetch", slug: "fetch", command: "uvx", args: ["mcp-server-fetch"], description: "Fetch web URLs and convert HTML to Markdown for the model.", homepage: "https://github.com/modelcontextprotocol/servers/tree/main/src/fetch", vendor: "modelcontextprotocol", category: "general" },
  { name: "Memory", slug: "memory", command: "npx", args: ["-y", "@modelcontextprotocol/server-memory"], description: "Persistent knowledge-graph memory across sessions.", homepage: "https://github.com/modelcontextprotocol/servers/tree/main/src/memory", vendor: "modelcontextprotocol", category: "ai" },
  { name: "Sequential Thinking", slug: "sequential-thinking", command: "npx", args: ["-y", "@modelcontextprotocol/server-sequential-thinking"], description: "Structured step-by-step reasoning scaffold for complex tasks.", homepage: "https://github.com/modelcontextprotocol/servers/tree/main/src/sequentialthinking", vendor: "modelcontextprotocol", category: "ai" },
  { name: "Time", slug: "time", command: "uvx", args: ["mcp-server-time"], description: "Current time, timezone conversion, and date arithmetic helpers.", homepage: "https://github.com/modelcontextprotocol/servers/tree/main/src/time", vendor: "modelcontextprotocol", category: "general" },
  // Code platforms
  { name: "GitHub", slug: "github", command: "docker", args: ["run", "-i", "--rm", "-e", "GITHUB_PERSONAL_ACCESS_TOKEN", "ghcr.io/github/github-mcp-server"], description: "Read and manage GitHub repos, issues, PRs, Actions, and code search.", env_note: "GITHUB_PERSONAL_ACCESS_TOKEN", homepage: "https://github.com/github/github-mcp-server", vendor: "github", category: "code" },
  { name: "GitLab", slug: "gitlab", command: "npx", args: ["-y", "@zereight/mcp-gitlab"], description: "Browse projects, MRs, issues, and pipelines on GitLab.", env_note: "GITLAB_PERSONAL_ACCESS_TOKEN", homepage: "https://github.com/zereight/gitlab-mcp", vendor: "gitlab", category: "code" },
  { name: "Sentry", slug: "sentry", command: "npx", args: ["-y", "@sentry/mcp-server"], description: "Query Sentry issues, events, and releases for debugging context.", env_note: "SENTRY_AUTH_TOKEN", homepage: "https://docs.sentry.io/product/sentry-mcp/", vendor: "sentry", category: "ops" },
  // Search
  { name: "Brave Search", slug: "brave", command: "npx", args: ["-y", "@modelcontextprotocol/server-brave-search"], description: "Web and local search via the Brave Search API.", env_note: "BRAVE_API_KEY", homepage: "https://github.com/modelcontextprotocol/servers-archived/tree/main/src/brave-search", vendor: "brave", category: "search" },
  { name: "Exa Search", slug: "exa", command: "npx", args: ["-y", "exa-mcp-server"], description: "Neural web search optimized for research and code retrieval.", env_note: "EXA_API_KEY", homepage: "https://github.com/exa-labs/exa-mcp-server", vendor: "exa", category: "search" },
  { name: "Tavily", slug: "tavily", command: "npx", args: ["-y", "tavily-mcp"], description: "AI-native search and extract API tuned for agent workflows.", env_note: "TAVILY_API_KEY", homepage: "https://github.com/tavily-ai/tavily-mcp", vendor: "tavily", category: "search" },
  { name: "Context7", slug: "context7", command: "npx", args: ["-y", "@upstash/context7-mcp"], description: "Pulls up-to-date, version-accurate docs for any library into context.", homepage: "https://github.com/upstash/context7", vendor: "upstash", category: "code" },
  // Data
  { name: "Postgres", slug: "postgres", command: "npx", args: ["-y", "@modelcontextprotocol/server-postgres", "postgresql://localhost/mydb"], description: "Read-only SQL access and schema introspection for Postgres.", homepage: "https://github.com/modelcontextprotocol/servers-archived/tree/main/src/postgres", vendor: "postgres", category: "data" },
  { name: "SQLite", slug: "sqlite", command: "uvx", args: ["mcp-server-sqlite", "--db-path", "./db.sqlite"], description: "Query and modify local SQLite databases.", homepage: "https://github.com/modelcontextprotocol/servers-archived/tree/main/src/sqlite", vendor: "sqlite", category: "data" },
  { name: "Supabase", slug: "supabase", command: "npx", args: ["-y", "@supabase/mcp-server-supabase"], description: "Manage Supabase projects, run SQL, and inspect schema/auth.", env_note: "SUPABASE_ACCESS_TOKEN", homepage: "https://github.com/supabase-community/supabase-mcp", vendor: "supabase", category: "data" },
  { name: "Neon", slug: "neon", command: "npx", args: ["-y", "@neondatabase/mcp-server-neon"], description: "Provision and query Neon serverless Postgres branches.", env_note: "NEON_API_KEY", homepage: "https://github.com/neondatabase/mcp-server-neon", vendor: "neon", category: "data" },
  // Ops / cloud
  { name: "Cloudflare", slug: "cloudflare", command: "npx", args: ["-y", "@cloudflare/mcp-server-cloudflare"], description: "Manage Workers, KV, R2, D1, and analytics on Cloudflare.", env_note: "CLOUDFLARE_API_TOKEN", homepage: "https://github.com/cloudflare/mcp-server-cloudflare", vendor: "cloudflare", category: "ops" },
  { name: "Vercel", slug: "vercel", command: "npx", args: ["-y", "@vercel/mcp-adapter"], description: "Deploy, inspect, and manage Vercel projects, env vars, and logs.", env_note: "VERCEL_TOKEN", homepage: "https://vercel.com/docs/mcp", vendor: "vercel", category: "ops" },
  { name: "AWS", slug: "aws", command: "uvx", args: ["awslabs.core-mcp-server"], description: "Query and operate AWS resources via the official AWS Labs server.", env_note: "AWS_PROFILE", homepage: "https://github.com/awslabs/mcp", vendor: "aws", category: "ops" },
  // Comms
  { name: "Slack", slug: "slack", command: "npx", args: ["-y", "@modelcontextprotocol/server-slack"], description: "Post messages, read channels, and search Slack history.", env_note: "SLACK_BOT_TOKEN", homepage: "https://github.com/modelcontextprotocol/servers-archived/tree/main/src/slack", vendor: "slack", category: "comms" },
  { name: "Linear", slug: "linear", command: "npx", args: ["-y", "mcp-remote", "https://mcp.linear.app/mcp"], description: "Create, search, and update Linear issues, projects, and cycles.", homepage: "https://linear.app/docs/mcp", vendor: "linear", category: "comms" },
  { name: "Notion", slug: "notion", command: "npx", args: ["-y", "@notionhq/notion-mcp-server"], description: "Read and write Notion pages, databases, and comments.", env_note: "NOTION_API_KEY", homepage: "https://github.com/makenotion/notion-mcp-server", vendor: "notion", category: "comms" },
  { name: "Atlassian (Jira + Confluence)", slug: "jira", command: "npx", args: ["-y", "mcp-remote", "https://mcp.atlassian.com/v1/sse"], description: "Search and manage Jira issues and Confluence pages.", homepage: "https://www.atlassian.com/platform/remote-mcp-server", vendor: "atlassian", category: "comms" },
  // Design / business
  { name: "Figma Dev Mode", slug: "figma", command: "npx", args: ["-y", "figma-developer-mcp", "--stdio"], description: "Pull frames, variables, and code-ready specs from Figma files.", env_note: "FIGMA_API_KEY", homepage: "https://github.com/GLips/Figma-Context-MCP", vendor: "figma", category: "design" },
  { name: "Stripe", slug: "stripe", command: "npx", args: ["-y", "@stripe/mcp", "--tools=all"], description: "Look up customers, payments, products, and invoices in Stripe.", env_note: "STRIPE_SECRET_KEY", homepage: "https://github.com/stripe/agent-toolkit", vendor: "stripe", category: "ops" },
  { name: "Mintlify Docs", slug: "search", command: "npx", args: ["-y", "mint-mcp"], description: "Search and retrieve content from any Mintlify-hosted docs site.", homepage: "https://mintlify.com/docs/mcp", vendor: "mintlify", category: "search" },
  // Browser automation
  { name: "Playwright", slug: "puppeteer", command: "npx", args: ["-y", "@playwright/mcp@latest"], description: "Drive a real browser to navigate, click, screenshot, and scrape.", homepage: "https://github.com/microsoft/playwright-mcp", vendor: "microsoft", category: "general" },
  { name: "Puppeteer", slug: "puppeteer", command: "npx", args: ["-y", "@modelcontextprotocol/server-puppeteer"], description: "Headless Chromium automation for scraping and page interaction.", homepage: "https://github.com/modelcontextprotocol/servers-archived/tree/main/src/puppeteer", vendor: "google", category: "general" },
  // Shell utility
  { name: "Shell", slug: "shell", command: "npx", args: ["-y", "mcp-server-shell"], description: "Run arbitrary shell commands with allow-list controls.", vendor: "community", category: "general" },
];

interface SkillListing {
  name: string;
  slug: string;
  description: string;
  category: "code" | "review" | "test" | "debug" | "docs" | "architect" | "security" | "ops";
  prompt: string;
}

const SKILL_LISTINGS: SkillListing[] = [
  { name: "Code Reviewer", slug: "code-reviewer", description: "Review diffs for bugs, regressions, and design issues.", category: "review",
    prompt: "You are a senior code reviewer. Examine the current diff (use `git diff` and `git log` if available) and identify correctness bugs, race conditions, missing error handling, security risks, and API/contract regressions. Group findings by severity (blocker, major, minor, nit), cite exact file:line, and propose a concrete fix for each. Do not restate what the code does; only call out what is wrong or risky." },
  { name: "Refactor Surgeon", slug: "refactor-surgeon", description: "Safe, behavior-preserving refactors in small steps.", category: "code",
    prompt: "You are a refactoring specialist. Make the smallest possible behavior-preserving change that achieves the user's goal. Before editing, state the refactor plan in one paragraph and identify the test or invariant that proves behavior is preserved. Prefer extracting, renaming, and inlining over rewrites; never combine refactors with feature changes in the same step." },
  { name: "Test Writer", slug: "test-writer", description: "Focused unit + integration tests targeting risky paths.", category: "test",
    prompt: "You are a pragmatic test author. Detect the project's existing test framework and conventions before writing anything. Generate tests that cover happy path, edge cases (empty, null, boundary, large input), and the most likely regressions; avoid trivial tests that only restate the implementation. Each test name must describe the behavior under test in plain English, and tests must be deterministic." },
  { name: "Bug Hunter", slug: "bug-hunter", description: "Reproduce, isolate, fix.", category: "debug",
    prompt: "You are a debugging specialist. Reproduce the reported bug first, in the smallest possible failing case, before changing any code. State the suspected root cause as a falsifiable hypothesis, then verify it (logs, prints, repro). Only after the root cause is confirmed do you propose a fix, and the fix must address the cause rather than the symptom." },
  { name: "Architect", slug: "tech-lead", description: "Plan features and systems before any code is written.", category: "architect",
    prompt: "You are a software architect. For the user's request, produce a short design doc covering: goals and non-goals, key data model, public interfaces, primary failure modes, and a phased implementation plan. Surface the two or three most important trade-offs explicitly and pick one with a stated reason. Do not write implementation code in this mode unless the user explicitly asks." },
  { name: "Security Auditor", slug: "security-auditor", description: "Audit for OWASP-class vulnerabilities and unsafe patterns.", category: "security",
    prompt: "You are a security auditor. Review the codebase or diff for OWASP-class issues: injection, auth/session flaws, secrets in code, SSRF, IDOR, unsafe deserialization, XSS, CSRF, and insecure defaults. For each finding, give severity, an exploit sketch in one sentence, the vulnerable file:line, and a concrete remediation. Never invent vulnerabilities; if unsure, mark the item as 'needs verification'." },
  { name: "Docs Writer", slug: "doc-writer", description: "Write and update READMEs, API docs, and comments.", category: "docs",
    prompt: "You are a documentation writer. Match the existing tone and structure of the project's docs. Write for a reader who has never seen this code: lead with what it is and why it exists, then how to use it, then reference detail. Every code example must be copy-pasteable and tested against the actual API; never invent flags or function signatures." },
  { name: "Performance Tuner", slug: "performance-tuner", description: "Find and fix hot-path performance and memory issues.", category: "debug",
    prompt: "You are a performance engineer. Before optimizing anything, measure: identify the slow path with a profile, benchmark, or timed log, and quote the number. Optimize only the proven hotspot, in order of impact, and re-measure after each change. Reject micro-optimizations that do not move the measured metric." },
  { name: "DevOps / Release", slug: "devops", description: "CI, deployments, and infra-as-code changes.", category: "ops",
    prompt: "You are a DevOps engineer. When changing CI, IaC, or deploy config, dry-run or lint the change locally before pushing (e.g. `terraform plan`, `actionlint`, `docker build`). Make changes idempotent and reversible, and never embed secrets in committed files. Call out blast radius (which environments, which services) before applying anything to production." },
  { name: "Migration Assistant", slug: "migrator", description: "Migrate code between framework or library versions.", category: "code",
    prompt: "You are a migration assistant. Locate the official upgrade guide and codemods for the target version before changing files, and apply codemods first. Migrate in small, compilable, test-passing increments; never mix unrelated cleanups into the migration commits. After each step, run typecheck and tests and stop on the first failure to diagnose." },
  { name: "API Designer", slug: "api-designer", description: "Design REST/RPC APIs with clear contracts and errors.", category: "architect",
    prompt: "You are an API designer. Define resources, verbs, request/response schemas, status codes, idempotency, pagination, and error shapes before writing handlers. Optimize for consumer ergonomics: stable names, predictable plurals, no leaking internals. Document every endpoint with an example request and example response, including at least one error case." },
  { name: "UI / Frontend Polisher", slug: "ux-reviewer", description: "Improve visual polish, accessibility, and microinteractions.", category: "code",
    prompt: "You are a senior frontend engineer focused on UI quality. Inspect the current component visually (screenshot if possible) and improve spacing, typography hierarchy, contrast, focus states, and motion. Respect the existing design system tokens; do not introduce new colors or fonts. Every interactive element must be keyboard accessible and have an appropriate ARIA role or label." },
];

const MCP_CATEGORIES: { id: McpListing["category"] | "all"; label: string }[] = [
  { id: "all", label: "All" },
  { id: "code", label: "Code" },
  { id: "search", label: "Search" },
  { id: "data", label: "Data" },
  { id: "comms", label: "Comms" },
  { id: "design", label: "Design" },
  { id: "ops", label: "Ops" },
  { id: "ai", label: "AI" },
];

export function MarketplaceView() {
  const [tab, setTab] = useState<Tab>("mcp");
  const [mcpCat, setMcpCat] = useState<McpListing["category"] | "all">("all");
  const [query, setQuery] = useState("");
  const [installed, setInstalled] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<Record<string, string>>({});

  useEffect(() => {
    api.listMcpServers().then((list) => {
      setInstalled(new Set(list.map((s) => s.name)));
    });
    api.getConfig().then((cfg) => {
      const skills = (cfg.extra?.skills as Record<string, unknown> | undefined) ?? {};
      setInstalled((prev) => {
        const next = new Set(prev);
        for (const k of Object.keys(skills)) next.add(`skill:${k}`);
        return next;
      });
    });
  }, []);

  const filteredMcp = useMemo(() => {
    const q = query.trim().toLowerCase();
    return MCP_LISTINGS.filter((m) => {
      if (mcpCat !== "all" && m.category !== mcpCat) return false;
      if (!q) return true;
      return (
        m.name.toLowerCase().includes(q) ||
        m.description.toLowerCase().includes(q) ||
        (m.vendor ?? "").toLowerCase().includes(q)
      );
    });
  }, [mcpCat, query]);

  const filteredSkills = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return SKILL_LISTINGS;
    return SKILL_LISTINGS.filter(
      (s) =>
        s.name.toLowerCase().includes(q) ||
        s.description.toLowerCase().includes(q) ||
        s.category.toLowerCase().includes(q),
    );
  }, [query]);

  const installMcp = async (m: McpListing) => {
    setBusy(m.slug);
    setError((e) => ({ ...e, [m.slug]: "" }));
    try {
      const expanded = m.args.map((a) => a.replaceAll("$HOME", "~"));
      await api.addMcpServer(m.name, m.command, expanded);
      setInstalled((s) => new Set(s).add(m.name));
    } catch (e) {
      setError((er) => ({ ...er, [m.slug]: String(e) }));
    } finally {
      setBusy(null);
    }
  };

  const installSkill = async (s: SkillListing) => {
    setBusy(s.slug);
    setError((e) => ({ ...e, [s.slug]: "" }));
    try {
      const cfg = await api.getConfig();
      const skills = (cfg.extra?.skills as Record<string, unknown> | undefined) ?? {};
      skills[s.slug] = { name: s.name, description: s.description, prompt: s.prompt };
      await api.setConfigField("skills", skills);
      setInstalled((set) => new Set(set).add(`skill:${s.slug}`));
    } catch (e) {
      setError((er) => ({ ...er, [s.slug]: String(e) }));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="flex flex-col h-full min-h-0 bg-[#0c0c12]">
      <div className="px-6 pt-5 pb-3 border-b border-white/[0.05]">
        <h1 className="text-[18px] font-semibold tracking-tight">Marketplace</h1>
        <p className="text-[12px] text-white/45 mt-0.5">
          {tab === "mcp" ? "Curated MCP servers — local tool integrations" : "Pre-tuned agent personas"}
        </p>
      </div>
      <div className="flex flex-col flex-1 min-h-0">
        {/* Tabs + search */}
        <div className="flex items-center gap-2 px-6 py-3 border-b border-white/[0.04]">
          <div className="flex items-center gap-1">
            <button
              onClick={() => setTab("mcp")}
              className={cn(
                "px-2.5 py-1 rounded-md text-[12px] transition-colors",
                tab === "mcp" ? "bg-white/[0.08] text-white" : "text-white/45 hover:text-white/75",
              )}
            >
              MCP Servers
              <span className="ml-1.5 text-[10px] text-white/40 tabular-nums">{MCP_LISTINGS.length}</span>
            </button>
            <button
              onClick={() => setTab("skills")}
              className={cn(
                "px-2.5 py-1 rounded-md text-[12px] transition-colors",
                tab === "skills" ? "bg-white/[0.08] text-white" : "text-white/45 hover:text-white/75",
              )}
            >
              Skills
              <span className="ml-1.5 text-[10px] text-white/40 tabular-nums">{SKILL_LISTINGS.length}</span>
            </button>
          </div>
          <div className="flex-1" />
          <div className="relative">
            <svg
              viewBox="0 0 16 16"
              width="12"
              height="12"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.6"
              strokeLinecap="round"
              className="absolute left-2 top-1/2 -translate-y-1/2 text-white/35"
            >
              <circle cx="7" cy="7" r="4.5" />
              <path d="M14 14l-3.4-3.4" />
            </svg>
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search…"
              className="h-7 w-[180px] pl-6 pr-2 rounded-md text-[12px] bg-white/[0.04] border border-white/[0.06] text-white/85 placeholder:text-white/30 focus:outline-none focus:bg-white/[0.07]"
            />
          </div>
        </div>

        {/* Category filter (mcp only) */}
        {tab === "mcp" && (
          <div className="flex items-center gap-1 px-6 py-2 border-b border-white/[0.04] overflow-x-auto vibn-scroll">
            {MCP_CATEGORIES.map((c) => (
              <button
                key={c.id}
                onClick={() => setMcpCat(c.id)}
                className={cn(
                  "px-2 py-0.5 rounded-full text-[11px] flex-shrink-0 transition-colors border",
                  mcpCat === c.id
                    ? "bg-violet-500/15 border-violet-400/30 text-violet-200"
                    : "border-transparent text-white/45 hover:text-white/75 hover:border-white/10",
                )}
              >
                {c.label}
              </button>
            ))}
          </div>
        )}

        {/* List */}
        <div className="flex-1 overflow-y-auto vibn-scroll px-6 py-4 flex flex-col gap-2">
          {tab === "mcp" ? (
            filteredMcp.length === 0 ? (
              <div className="text-center text-[12px] text-white/40 py-8">No servers match.</div>
            ) : (
              filteredMcp.map((m) => {
                const isInstalled = installed.has(m.name);
                return (
                  <div
                    key={m.slug + m.name}
                    className="rounded-lg bg-white/[0.025] border border-white/[0.06] p-3 flex gap-3 items-start"
                  >
                    <McpIcon slug={m.slug} className="h-9 w-9 flex-shrink-0" />
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2 flex-wrap">
                        <span className="text-[13px] font-semibold">{m.name}</span>
                        {m.vendor && (
                          <span className="text-[10px] text-white/35 font-mono">{m.vendor}</span>
                        )}
                        {m.env_note && (
                          <span className="text-[10px] text-amber-300/70 bg-amber-500/[0.08] border border-amber-500/20 rounded px-1.5 py-px font-mono">
                            needs {m.env_note}
                          </span>
                        )}
                      </div>
                      <div className="text-[11.5px] text-white/55 mt-0.5">{m.description}</div>
                      <div className="text-[10.5px] text-white/30 mt-0.5 truncate font-mono">
                        {m.command} {m.args.join(" ")}
                      </div>
                      {error[m.slug] && (
                        <div className="mt-1.5 text-[10.5px] text-red-300/80">{error[m.slug]}</div>
                      )}
                    </div>
                    <div className="flex-shrink-0">
                      {isInstalled ? (
                        <span className="text-[11px] text-emerald-300/80 font-mono">installed</span>
                      ) : (
                        <Button
                          variant="primary"
                          size="sm"
                          loading={busy === m.slug}
                          onClick={() => installMcp(m)}
                        >
                          Install
                        </Button>
                      )}
                    </div>
                  </div>
                );
              })
            )
          ) : filteredSkills.length === 0 ? (
            <div className="text-center text-[12px] text-white/40 py-8">No skills match.</div>
          ) : (
            filteredSkills.map((s) => {
              const isInstalled = installed.has(`skill:${s.slug}`);
              return (
                <div
                  key={s.slug}
                  className="rounded-lg bg-white/[0.025] border border-white/[0.06] p-3 flex gap-3 items-start"
                >
                  <SkillIcon slug={s.slug} className="h-9 w-9 flex-shrink-0" />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-[13px] font-semibold">{s.name}</span>
                      <span className="text-[10px] text-white/35 font-mono">{s.category}</span>
                    </div>
                    <div className="text-[11.5px] text-white/55 mt-0.5">{s.description}</div>
                    {error[s.slug] && (
                      <div className="mt-1.5 text-[10.5px] text-red-300/80">{error[s.slug]}</div>
                    )}
                  </div>
                  <div className="flex-shrink-0">
                    {isInstalled ? (
                      <span className="text-[11px] text-emerald-300/80 font-mono">installed</span>
                    ) : (
                      <Button
                        variant="primary"
                        size="sm"
                        loading={busy === s.slug}
                        onClick={() => installSkill(s)}
                      >
                        Install
                      </Button>
                    )}
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
}
