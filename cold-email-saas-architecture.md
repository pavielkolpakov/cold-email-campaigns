# Cold Email SaaS — Tech & Architecture Recommendation

## Recommended Stack

- **Frontend:** Next.js + TypeScript
- **Backend:** Rust + Axum
- **Async runtime:** Tokio
- **Database:** PostgreSQL
- **DB access:** SQLx
- **Queue / scheduling:** PostgreSQL-backed jobs initially
- **Optional later:** Redis for higher-throughput queues, caching, or rate limiting
- **Email providers:** Gmail API, Microsoft Graph, SMTP + IMAP
- **Storage:** S3-compatible object storage if needed
- **Deployment:** Simple app hosting first; no Kubernetes or microservices initially

## Architecture

```text
Next.js Web App
      |
      v
Rust API (Axum)
      |
      +------------------+
      |                  |
      v                  v
PostgreSQL          Email Providers
      |             - Gmail API
      |             - Microsoft Graph
      |             - SMTP / IMAP
      v
Rust Workers (Tokio)
- campaign sending
- followups
- mailbox sync
- reply detection
- bounce handling
- warmup scheduling
- rate limiting
```

## Job Processing

Keep PostgreSQL as the source of truth for scheduled sends and followups.

Use a jobs/messages table with fields such as:

- `scheduled_at`
- `status`
- `locked_at`
- `locked_by`
- `attempts`
- `sent_at`
- `last_error`

Workers can claim jobs using `FOR UPDATE SKIP LOCKED` or a lease-based pattern. This keeps the architecture durable and avoids adding Redis too early.

## Email Provider Abstraction

Use a common Rust trait for providers so campaign logic is independent of Gmail, Outlook, or SMTP.

```rust
trait MailProvider {
    async fn send(...);
    async fn fetch_messages(...);
    async fn refresh_auth(...);
}
```

Implementations:

- `GmailProvider`
- `MicrosoftProvider`
- `SmtpProvider`

## Why Rust Fits

Rust is a good fit because the system is mostly long-running, I/O-heavy concurrent workloads:

- many mailbox connections
- scheduled sends
- provider API calls
- inbox synchronization
- retries and timeouts
- rate limiting

Rust also gives strong compile-time guarantees, which works well with coding agents because `cargo check` and tests provide precise feedback.

## Initial Deployment Recommendation

Start with only:

1. Next.js frontend
2. One Rust backend/service
3. PostgreSQL

The Rust service can run different modes such as:

```text
app api
app worker
app scheduler
```

Avoid Kubernetes, Kafka, microservices, and complex infrastructure until scale actually requires them.
