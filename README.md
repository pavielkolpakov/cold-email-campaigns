# Cold Email Campaigns

Multi-tenant portal for running cold email campaigns from your own mailboxes.
See [PLAN.md](PLAN.md) for scope and phases.

```
api/   Rust + Axum + SQLx. One binary, three modes: api | worker | scheduler.
web/   Next.js (App Router) + TypeScript + Tailwind + shadcn/ui.
```

## Local development

```bash
cp .env.example .env          # then fill in ENCRYPTION_KEY: openssl rand -base64 32
docker compose up -d          # Postgres on :5433, Mailpit on :8025

cd api && cargo run -- api    # http://localhost:8080, runs migrations on boot
cd web && npm install && npm run dev   # http://localhost:3000
```

The frontend proxies `/api/*` to the Rust service, so the session cookie is
always same-origin. `API_URL` in `web/.env.local` points at the backend.

## Tests

```bash
cd api && cargo test           # each test gets its own throwaway database
```

## Database changes

Migrations live in `api/migrations` and are embedded into the binary. After
adding one, refresh the offline query cache so builds work without a database:

```bash
cd api
sqlx migrate run
cargo sqlx prepare -- --all-targets
```

## Connecting a Gmail mailbox

Mailbox connection needs a Google OAuth client. Until one is configured the
portal shows a clear "google oauth is not configured" error instead of failing
silently.

1. Create a project in the Google Cloud Console.
2. Configure the OAuth consent screen (External). Add yourself as a test user —
   the app stays limited to 100 test users until Google verifies it.
3. Add these scopes: `gmail.send`, `gmail.readonly`, `userinfo.email`.
   The first two are **restricted** and need verification before public use.
4. Create an OAuth client ID of type *Web application* with the redirect URI
   `http://localhost:3000/api/mailboxes/gmail/callback`.
5. Put the client id and secret in `.env`:

```
GOOGLE_CLIENT_ID=...
GOOGLE_CLIENT_SECRET=...
```

Tokens are encrypted with `ENCRYPTION_KEY` before they touch the database.

## Deployment (Railway)

Project `cold-email-campaigns` runs five services in `production`:

| Service   | What it is                | Notes |
|-----------|---------------------------|-------|
| Postgres  | managed database          | private networking only, no public URL |
| api       | `api` mode                | public domain, runs migrations on boot |
| worker    | `worker` mode             | sends and syncs inboxes |
| scheduler | `scheduler` mode          | enqueues due steps, sweeps stale jobs |
| web       | Next.js                   | public domain, proxies `/api/*` to `api` over private networking |

Deploys go from the local checkout, one subdirectory per service:

```bash
railway up ./api --path-as-root --service api       --detach -m "api"
railway up ./api --path-as-root --service worker    --detach -m "worker"
railway up ./api --path-as-root --service scheduler --detach -m "scheduler"
railway up ./web --path-as-root --service web       --detach -m "web"
```

`--path-as-root` matters: without it the CLI uploads from the git root and the
builder sees a monorepo it cannot identify.

The Rust binary picks its mode from `MODE`, set per service. `api` binds `PORT`
(8080), which `web` reaches at `http://api.railway.internal:8080`.
