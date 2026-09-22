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
