# Cold Email Campaigns Portal

## Overview

A multi-tenant web portal for running cold email campaigns from your own mailboxes.
An org connects a Gmail account, imports leads from a CSV, builds a multi-step
sequence with delays and merge tags, and launches a campaign. A Rust scheduler and
worker send on a human-looking schedule inside per-mailbox limits, then sync the
inbox to detect replies and automatically stop followups to anyone who answered.
Built for teams running their own outbound; not a mail-blasting relay.

## MVP Features

- **Org & auth** — email/password signup creating an org, login, session cookie, invite teammates to the org.
- **Mailbox connection** — Gmail OAuth connect/disconnect, encrypted token storage, automatic refresh, per-mailbox send test.
- **Lead import** — CSV upload with column mapping to `email`, `first_name`, `last_name`, `company` + arbitrary custom fields; dedupe by email within the org; named lead lists.
- **Sequence builder** — ordered steps, each with subject, body, and a delay in days from the previous step; merge tags (`{{first_name}}`) resolved from lead fields with per-tag fallbacks; followups sent as replies on the original thread.
- **Campaign** — pick a sequence + lead list + mailbox, launch, pause, resume.
- **Sending engine** — scheduler enqueues due steps; workers claim jobs, render, and send via Gmail; per-mailbox daily cap, randomized delay between sends, business-hours window in the lead's timezone.
- **Reply detection** — periodic Gmail sync per mailbox, inbound matched to campaign by thread, lead marked replied, all pending jobs for that lead cancelled.
- **Stats** — per campaign: sent, opens deferred, replied, bounced, remaining; per-lead activity timeline.
- **Suppression** — one-click unsubscribe link, org-wide suppression list checked before every send.

## Out of Scope (v1)

- Billing, plans, usage metering.
- Microsoft Graph and SMTP/IMAP providers (the `MailProvider` trait is built so they drop in later).
- Mailbox warmup scheduling.
- Multi-mailbox rotation within one campaign.
- Open/click tracking pixels and link rewriting.
- A/B variants, spintax, AI-generated copy.
- Unified inbox / reply-from-portal UI — replies are read in Gmail for v1.
- CRM integrations, webhooks, public API.
- Deliverability dashboard (SPF/DKIM/DMARC checks, spam-score preview).

## Tech Stack

| Layer        | Choice                                  | Reason |
|--------------|-----------------------------------------|--------|
| Frontend     | Next.js (App Router) + TypeScript       | Server components keep data fetching close to the API; one language for the whole UI. |
| UI kit       | Tailwind + shadcn/ui                    | Fast to a credible dashboard without designing primitives. |
| Backend      | Rust + Axum + Tokio                     | Workload is long-running, I/O-heavy concurrency; `cargo check` gives tight feedback loops. |
| DB access    | SQLx (compile-time checked queries)     | Query errors surface at build time, not in production sends. |
| Database     | PostgreSQL                              | Source of truth for data *and* the job queue. |
| Queue        | Postgres jobs table, `FOR UPDATE SKIP LOCKED` | Durable, transactional with the data it schedules, no extra infra. Redis only when throughput demands it. |
| Auth         | Axum-owned sessions, argon2, httpOnly cookie | Single source of truth; workers can act on behalf of a user without token-sync between two frameworks. |
| Email        | Gmail API (OAuth) behind a `MailProvider` trait | First provider; Graph and SMTP implement the same trait later. |
| Secrets      | AES-GCM envelope encryption for OAuth tokens | Refresh tokens are the crown jewels; never stored in plaintext. |
| Hosting      | Railway (Postgres + Rust service + Next.js) | Simple app hosting as the architecture doc recommends; no Kubernetes. |
| Local dev    | docker-compose (Postgres + Mailpit)     | Mailpit catches sends so tests never touch a real inbox. |

## Architecture

```text
                    Browser
                       |
                       v
              Next.js (App Router)
                       |  fetch, session cookie forwarded
                       v
              Rust binary: mode = api
              Axum + SQLx + tower middleware
                       |
        +--------------+--------------+
        |                             |
        v                             v
   PostgreSQL                    Gmail API
  - orgs, users, sessions        - send
  - mailboxes (enc. tokens)      - history.list / messages.get
  - leads, lead_lists            - OAuth refresh
  - sequences, steps
  - campaigns, campaign_leads
  - messages, events
  - jobs  <-- queue
        ^
        |  claim / complete
        |
  +-----+--------------------------------+
  |                                      |
Rust binary: mode = scheduler     Rust binary: mode = worker
- enqueue due sequence steps      - send jobs (render, cap, jitter, hours)
- enqueue mailbox sync jobs       - sync jobs (fetch inbox, match threads)
- requeue stale leases            - cancel followups on reply/bounce
```

One binary, three modes, selected by argv — `app api`, `app worker`, `app scheduler`.
Deployed as three Railway services off the same image.

### Data model sketch

```
orgs(id, name, created_at)
users(id, org_id, email, password_hash, role, created_at)
sessions(id, user_id, expires_at)
mailboxes(id, org_id, provider, email, access_token_enc, refresh_token_enc,
          token_expires_at, daily_cap, sent_today, sending_window, timezone, status)
lead_lists(id, org_id, name)
leads(id, org_id, list_id, email, first_name, last_name, company,
      custom jsonb, timezone, status, unsubscribed_at)
sequences(id, org_id, name)
sequence_steps(id, sequence_id, position, delay_days, subject, body)
campaigns(id, org_id, sequence_id, list_id, mailbox_id, status, started_at)
campaign_leads(id, campaign_id, lead_id, current_step, next_run_at, status,
               thread_id, replied_at, bounced_at)
messages(id, campaign_lead_id, step_id, provider_message_id, thread_id,
         direction, subject, body, sent_at)
jobs(id, org_id, kind, payload jsonb, scheduled_at, status,
     locked_at, locked_by, attempts, last_error)
suppressions(id, org_id, email, reason, created_at)
events(id, org_id, entity_type, entity_id, kind, data jsonb, created_at)
```

Every table below `orgs` carries `org_id`. Tenant isolation is enforced in one
place — a repository layer that refuses to build a query without an org scope —
and backed by Postgres RLS policies as a second line.

## Implementation Phases

### Phase 1 — Foundation
- [ ] Monorepo scaffold: `/api` (cargo), `/web` (Next.js), `/migrations`, root `docker-compose.yml` with Postgres + Mailpit.
- [ ] SQLx migration runner wired into startup; first migration creates `orgs`, `users`, `sessions`.
- [ ] Axum skeleton with mode dispatch (`api` / `worker` / `scheduler`), structured logging, `/health`.
- [ ] Signup (creates org + owner user), login, logout; argon2 hashing; httpOnly session cookie.
- [ ] Auth extractor that yields `(user, org_id)`; org-scoped repository layer; RLS policies.
- [ ] Next.js app shell: login/signup pages, authenticated layout, nav, empty dashboard.
- [ ] Railway project: Postgres, api service, web service; env config; deploy.
- [ ] **Verify:** signup → login → dashboard works on the live Railway URL; a second org cannot read the first org's rows (integration test).

### Phase 2 — Mailboxes & Leads
- [x] `MailProvider` trait (`send`, `fetch_messages`, `refresh_auth`) + `GmailProvider`.
- [ ] Google Cloud project, OAuth consent screen, scopes `gmail.send` + `gmail.readonly`.
- [x] OAuth connect flow with state param; token encryption at rest; refresh-on-expiry helper.
- [x] Mailbox list UI: connect, disconnect, status, daily cap. *(Sending window and timezone move to Phase 3, where the scheduler actually reads them.)*
- [x] "Send test email" action proving the round trip.
- [x] CSV upload: parse, preview, map columns to fields, import with per-org dedupe on email; unmapped columns land in `custom`.
- [x] Lead list + lead table UI. *(Search and status filter deferred; nothing to filter yet.)*
- [ ] **Verify:** connect a real Gmail account, send a test email to yourself, import a 100-row CSV with a custom column and see it on the lead detail page.

### Phase 3 — Sequences & Sending
- [ ] Sequence CRUD; step editor with subject/body, delay in days, reorder.
- [ ] Merge-tag renderer with fallbacks; live preview against a sample lead; validation rejects unknown tags at save time.
- [ ] Suppression list + unsubscribe token route; every send checks it. *(Pulled forward from polish — it is a legal requirement, not a nicety.)*
- [ ] Campaign create/launch: snapshot lead list into `campaign_leads`, set step 0 `next_run_at`.
- [ ] Scheduler mode: poll for due `campaign_leads`, insert `send` jobs, honour pause.
- [ ] Worker mode: claim with `FOR UPDATE SKIP LOCKED` + lease, render, enforce daily cap / business-hours window / random jitter, send via provider, write `messages` + `events`, advance to next step.
- [ ] Retry with exponential backoff; permanent vs transient error classification; stale-lease requeue.
- [ ] Campaign detail UI: status, progress, pause/resume, per-lead state.
- [ ] **Verify:** a 3-step sequence to a handful of seed inboxes sends step 1 immediately and steps 2–3 at the configured delays, never exceeding the daily cap, never outside the window; killing the worker mid-run loses no jobs.

### Phase 4 — Reply Detection
- [ ] Gmail history sync per mailbox, cursor persisted; enqueued by the scheduler on an interval.
- [ ] Match inbound messages to `campaign_leads` by `thread_id`; ignore auto-replies (`Auto-Submitted`, vacation headers).
- [ ] Bounce detection from delivery-status notifications; mark lead bounced and suppress.
- [ ] On reply or bounce: mark the lead, cancel pending jobs for that lead, emit an event.
- [ ] Followups send as replies on the original thread (`In-Reply-To` / `References`).
- [ ] Campaign stats: sent, replied, bounced, remaining; per-lead activity timeline.
- [ ] **Verify:** reply from a seed inbox → within one sync cycle the lead shows `replied`, its queued followups are cancelled, and campaign stats update.

### Phase 5 — Polish & Launch
- [ ] Error surfaces: mailbox disconnected / quota exceeded / token revoked shown in the UI with a fix action.
- [ ] Gmail rate-limit handling with backoff and per-mailbox concurrency ceiling.
- [ ] Team invites and a basic owner/member role split.
- [ ] Dashboard: active campaigns, today's sends against cap, recent replies.
- [ ] Seed script + end-to-end test against Mailpit covering the full loop.
- [ ] Google OAuth verification submission for the restricted scopes.
- [ ] **Verify:** full loop green in CI against Mailpit; a revoked token produces a clear UI state instead of silent failure.

## Open Questions

- **Google verification timing.** `gmail.send` and `gmail.readonly` are restricted scopes; publishing needs Google review plus a security assessment, which can run weeks. Through Phase 4 the app stays in testing mode (100 test users). Submit at Phase 5 or earlier if the launch date is fixed.
- **Sync cadence.** Polling Gmail history every 1–2 minutes per mailbox is simple and cheap at low mailbox counts. Gmail push notifications (Pub/Sub watch) are lower latency but add a public webhook endpoint and renewal logic. Start with polling; revisit when mailbox count makes it expensive.
- **Lead timezone source.** Business-hours sending needs a timezone per lead. Options: a CSV column, inference from country, or falling back to the mailbox's timezone. v1 falls back to the mailbox; revisit if it matters.
- **Reply matching without thread IDs.** Fine for Gmail. When SMTP/IMAP lands, matching will need `Message-ID` / `References` header tracking — worth keeping those columns on `messages` from the start.
- **Open tracking.** Deliberately out of v1: tracking pixels hurt deliverability and Apple Mail Privacy Protection makes open rates close to meaningless. Revisit only if a customer demands the number.
- **Sending concurrency ceiling.** How many mailboxes one worker process should handle before adding a second Railway replica — measure at Phase 3 rather than guessing now.
