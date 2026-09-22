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
- [x] Monorepo scaffold: `/api` (cargo), `/web` (Next.js), `/migrations`, root `docker-compose.yml` with Postgres + Mailpit.
- [x] SQLx migration runner wired into startup; first migration creates `orgs`, `users`, `sessions`.
- [x] Axum skeleton with mode dispatch (`api` / `worker` / `scheduler`), structured logging, `/health`.
- [x] Signup (creates org + owner user), login, logout; argon2 hashing; httpOnly session cookie.
- [x] Auth extractor that yields `(user, org_id)`; org-scoped repository layer. *(RLS policies still open — see Open Questions.)*
- [x] Next.js app shell: login/signup pages, authenticated layout, nav, empty dashboard.
- [ ] Railway project: Postgres, api service, web service; env config; deploy.
- [x] **Verify (local only):** signup → login → dashboard confirmed in a browser; cross-org isolation covered by tests (`separate_signups_get_separate_orgs`, and the mailbox/lead isolation tests in Phase 2). *Not yet confirmed on a live Railway URL — nothing is deployed.*

### Phase 2 — Mailboxes & Leads
- [x] `MailProvider` trait (`send`, `fetch_messages`, `refresh_auth`) + `GmailProvider`.
- [x] Google Cloud project, OAuth consent screen, scopes `gmail.send` + `gmail.readonly`. *(Project 591030765458, in Testing mode. Two gotchas hit: the connecting account must be on the Test users list, and the Gmail API must be enabled separately from OAuth — neither fails until you try to send.)*
- [x] OAuth connect flow with state param; token encryption at rest; refresh-on-expiry helper.
- [x] Mailbox list UI: connect, disconnect, status, daily cap. *(Sending window and timezone move to Phase 3, where the scheduler actually reads them.)*
- [x] "Send test email" action proving the round trip.
- [x] CSV upload: parse, preview, map columns to fields, import with per-org dedupe on email; unmapped columns land in `custom`.
- [x] Lead list + lead table UI. *(Search and status filter deferred; nothing to filter yet.)*
- [x] **Verify (live):** `qwp.kpv@gmail.com` connected via real OAuth; test email delivered through the Gmail API; CSV imported with custom columns visible on the lead page. *Import was exercised with a 6-row file covering duplicates, a malformed address and a missing address — not a 100-row file, so bulk behaviour at size is still unproven.*

### Phase 3 — Sequences & Sending
- [x] Sequence CRUD; step editor with subject/body, delay in days, reorder.
- [x] Merge-tag renderer with fallbacks. *(Rendering refuses at send time when a tag has no value and no fallback, rather than validating at save — the lead data is what decides, and it is not known when the sequence is written. Live preview deferred.)*
- [x] Suppression list + unsubscribe token route; every send checks it. *(Pulled forward from polish — it is a legal requirement, not a nicety.)*
- [x] Campaign create/launch: snapshot lead list into `campaign_leads`, set step 0 `next_run_at`.
- [x] Scheduler mode: poll for due `campaign_leads`, insert `send` jobs, honour pause.
- [x] Worker mode: claim with `FOR UPDATE SKIP LOCKED`, render, enforce daily cap, send via provider, write `messages`, advance to next step. *(Business-hours window and random jitter still open — see Open Questions.)*
- [x] Retry with exponential backoff; permanent vs transient error classification. *(Stale-lease requeue still open.)*
- [x] Campaign detail UI: status, stats, launch/pause/resume. *(Per-lead drill-down deferred.)*
- [x] **Verify (live, partial):** a 2-step sequence sent through real Gmail from the real `worker` and `scheduler` processes; step 1 and its followup both delivered, sharing one thread.

  **This run found a real bug.** The followup opened a *new* conversation: Gmail threads on the RFC822 `Message-ID` header, not the API's `threadId`, and also needs `threadId` in the send body. We now stamp our own `Message-ID`, store it, and point the followup's `In-Reply-To`/`References` at it. The pre-existing test asserted the broken contract, so it passed throughout — a fake mailer confirms whatever you tell it to.

  **Still unproven:** multi-day delays (the live run used delay 0), the daily cap under real sending, and killing a worker mid-send. All three are covered by tests against the fake mailer only.

### Phase 4 — Reply Detection
- [x] Gmail history sync per mailbox, cursor persisted. *(Claimed straight off the `mailboxes` row rather than through the job queue: a sync is idempotent and there is at most one outstanding per mailbox, so a stamped `last_synced_at` is the whole lock. An expired history cursor — Gmail keeps about a week — restarts from the current position instead of failing forever.)*
- [x] Match inbound messages to `campaign_leads` by `thread_id`; ignore auto-replies (`Auto-Submitted`, `X-Autoreply`, `Precedence`). Our own sent copy in the thread is ignored too.
- [x] Bounce detection from delivery-status notifications; mark lead bounced and suppress the address. *(Bounces are classified before auto-replies: they often carry both headers, and a dead address matters more.)*
- [x] On reply or bounce: mark the lead, cancel queued jobs for that lead.
- [x] Followups send as replies on the original thread. *(Done in Phase 3, after the live run showed it was broken.)*
- [x] Campaign stats: sent, replied, bounced, remaining. *(Per-lead activity timeline deferred — inbound messages are not stored, only the id of the one that stopped the lead.)*
- [x] **Verify (live):** replied to a real campaign email from Gmail; the worker marked the lead `replied` within one sync cycle (`inbox synced replies=1`), cleared `next_run_at`, and the followup — which said "IF YOU ARE READING THIS, REPLY DETECTION FAILED" — never sent. *Auto-reply and bounce paths are covered by tests only; neither has been seen against real Gmail.*

### Phase 5 — Polish & Launch
- [x] Error surfaces: disconnected-mailbox banner with a Reconnect action on the dashboard and mailboxes pages; a "needs attention" list of leads that failed and why; provider failures return 502 carrying Google's own message; an unreachable API names itself.
- [x] Gmail rate-limit handling with backoff. *(A 429, or a 403 with a rate-limit reason, defers the job by `Retry-After` and gives back the attempt it spent — throttling is not the lead's fault. A 400 fails that lead outright, since a malformed address never improves. A 401 mid-send disconnects the mailbox.)*
- [x] Team invites and a basic owner/member role split. *(Only an owner may invite or revoke. The invite carries the org and the email, so accepting cannot land someone in a different org or under a different address.)*
- [x] Dashboard: active campaigns, today's sends against cap, recent replies, and what needs attention.
- [x] Stale-lease sweeper: jobs abandoned by a dead worker return to the queue; ones out of attempts fail rather than loop. *(Was an open gap from Phase 3.)*
- [x] Seed script — `cargo run -- seed` builds a demo org, leads and a sequence through the same functions the app uses, so it cannot drift.
- [ ] Google OAuth verification submission for the restricted scopes. *(Yours to submit; app is still in Testing mode.)*
- [x] **Verify (live):** invite created, accepted in a browser, joined the right org as a member, and the Team page refuses them. Rate-limit, invalid-recipient and mid-send revocation are covered by tests against a failing mailer.

  **Dropped: the Mailpit end-to-end test.** Mailpit speaks SMTP and v1 has no SMTP provider — the plan carried this over from a stack assumption that the Gmail-only decision invalidated. The full loop is already covered end to end against the in-memory mailer, and against real Gmail by hand. Revisit when `SmtpProvider` lands.

## Verified against live Gmail

What has actually been exercised end to end, as opposed to passing against the
in-memory fake:

| Path | Status |
|---|---|
| OAuth connect, consent, token storage | live |
| Test send through the Gmail API | live |
| Token refresh on expiry | **fake/wiremock only** |
| Revoked grant → mailbox disconnected | **wiremock only** |
| Worker + scheduler as real processes | live |
| Two-step campaign, merge tags on real data | live |
| Followup threading | live — and this is where the bug was |
| Reply detected, followup cancelled | live |
| Auto-reply ignored, bounce suppressed | **tests only** |
| Gmail history cursor expiry / restart | **not verified** |
| Multi-day delays, daily cap, worker kill | **tests only** |
| Unsubscribe link through a real mail client | **not verified** |
| SPF/DKIM/DMARC, spam placement | **not verified** |
| Invite created, accepted, role enforced | live |
| Rate limit, invalid recipient, mid-send revocation | **tests only** |

## Open Questions

- **Google verification timing.** `gmail.send` and `gmail.readonly` are restricted scopes; publishing needs Google review plus a security assessment, which can run weeks. Through Phase 4 the app stays in testing mode (100 test users). Submit at Phase 5 or earlier if the launch date is fixed.
- **Sync cadence.** Polling Gmail history every 1–2 minutes per mailbox is simple and cheap at low mailbox counts. Gmail push notifications (Pub/Sub watch) are lower latency but add a public webhook endpoint and renewal logic. Start with polling; revisit when mailbox count makes it expensive.
- **Lead timezone source.** Business-hours sending needs a timezone per lead. Options: a CSV column, inference from country, or falling back to the mailbox's timezone. v1 falls back to the mailbox; revisit if it matters.
- **Reply matching without thread IDs.** Fine for Gmail. When SMTP/IMAP lands, matching will need `Message-ID` / `References` header tracking — worth keeping those columns on `messages` from the start.
- **Open tracking.** Deliberately out of v1: tracking pixels hurt deliverability and Apple Mail Privacy Protection makes open rates close to meaningless. Revisit only if a customer demands the number.
- **Business-hours window and jitter are not implemented.** The worker sends as soon as a job is claimed. Both need a timezone to be meaningful, and the lead timezone question below is still open; doing it against the mailbox timezone alone is a half-measure worth deciding on deliberately.
- **The daily cap counts sends across the whole day, not a rolling window,** and resets at UTC midnight rather than the mailbox's local midnight.
- **Postgres RLS.** Still unimplemented. `force row level security` does not apply to superusers, and the local/Railway Postgres role is one, so enforcing it needs a separate non-superuser app role (migrations as owner, runtime as `app`) and a second test pool. Org scoping is currently enforced in the repository layer and covered by tests; decide before Phase 3 whether the second line of defence is worth that plumbing.
- **Import writes one row per query.** Fine for the list sizes seen so far; becomes the bottleneck somewhere in the thousands. Batch inserts when it shows up in practice, not before.
- **A lead belongs to exactly one list.** `leads` carries `list_id` and is unique per org, so importing the same address into a second list reports it as a duplicate rather than adding it twice. If a prospect needs to sit in several lists, that becomes a join table.
- **Sending concurrency ceiling.** Not built. A worker processes its claimed jobs one at a time, so a single worker is already a ceiling of one send at a time per process; a real per-mailbox limit across several workers needs coordination none of this has yet. Measure before building it.
- **Invites are not emailed.** The owner copies a link and sends it themselves. Mailing it would mean sending transactional email from the product's own domain, which is a separate sender identity from the customer's connected mailbox.
