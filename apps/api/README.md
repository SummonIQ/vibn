# vibn-auth-api

Better Auth API for the vibn Tauri desktop app. Next.js 15 (App Router) on Vercel, backed by Neon Postgres.

## Setup

```bash
npm install
npx prisma generate
```

Then either:

**Use Neon via Vercel** (recommended):

```bash
vercel link
vercel env pull .env.local
npx prisma db push
npm run dev
```

**Or use a local Postgres**:

```bash
docker run --name vibn-pg -e POSTGRES_PASSWORD=postgres -p 5432:5432 -d postgres
# Then in .env:
#   DATABASE_URL="postgresql://postgres:postgres@localhost:5432/postgres"
#   DIRECT_URL="postgresql://postgres:postgres@localhost:5432/postgres"
#   BETTER_AUTH_SECRET="$(openssl rand -base64 32)"
#   BETTER_AUTH_URL="http://localhost:3000"
npx prisma db push
npm run dev
```

## Endpoints

- `GET  /api/health` — `{ ok: true }` health check
- `POST /api/auth/sign-up/email`
- `POST /api/auth/sign-in/email`
- `POST /api/auth/sign-out`
- `GET  /api/auth/get-session`
- (all other Better Auth routes under `/api/auth/*`)

## Trusted origins (Tauri)

Configured in `lib/auth.ts`:

- `vibn://` — production custom scheme
- `tauri://localhost` — macOS/Linux production webview
- `http://tauri.localhost` — Windows production webview
- `http://localhost:1420` — Tauri dev server
