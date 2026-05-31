# apps/marketing

Marketing site for **Vibn** — a local AI coding agent. Lives at https://vibn.dev.

Part of the Vibn monorepo. Sibling JS/TS apps live alongside under `apps/`. Rust crates (`vibn-core`, `vibn`, `vibn-desktop`) live at the repo root.

## Stack

- Next.js 16 (App Router) + React 19
- Tailwind CSS v4 (PostCSS plugin, theme in `app/globals.css`)
- Framer Motion for entrances and transitions
- Geist Sans / Geist Mono
- Bun for install + dev
- Deployed on Vercel, domain `vibn.dev`

## Layout

- `app/page.tsx` — desktop-app marketing (main landing)
- `app/cli/page.tsx` — Vibn CLI sub-page (terminal-first vibe)
- `app/components/*` — shared homepage components (`hero`, `features`, `models`, `tools`, `cta`, `site-header`, `site-footer`, `logo`)
- `app/cli/_components/*` — CLI page sections (`cli-hero`, `install`, `cli-features`, `slash-commands`)

## Hero concept

The desktop hero (`app/components/hero.tsx`) is a 3D-perspective MacBook frame that tilts on mouse parallax. The screen contains a live Vibn UI mock and a canvas particle system **confined inside the screen bounds** — particles bounce off the screen edges. Visual metaphor for "your data never leaves your machine."

The CLI hero (`app/cli/_components/cli-hero.tsx`) is a full-bleed canvas particle system with a terminal mock that types out a real-looking session, plus an inline copyable `cargo install vibn`.

Both heroes respect `prefers-reduced-motion`.

## Conventions

- Colors live in `app/globals.css` under `@theme` — reference via `var(--color-*)` or Tailwind utilities like `bg-[color:var(--color-violet)]`.
- Keep marketing copy concrete: name actual tools, models, file paths. The product is unusual; specificity sells it.
- Animation library is **Framer Motion** for component-level orchestration. Canvas + vanilla `requestAnimationFrame` for the heroes.
- No backend, no auth, no database. Static deploy target.

## Commands

```bash
bun install
bun dev      # http://localhost:30100
bun build
bun typecheck
```
