-- Sequences, campaigns, the per-lead state machine, and the job queue.

create table sequences (
    id         uuid primary key default gen_random_uuid(),
    org_id     uuid        not null references orgs (id) on delete cascade,
    name       text        not null,
    created_at timestamptz not null default now()
);

create index sequences_org_id_idx on sequences (org_id);

create table sequence_steps (
    id          uuid primary key default gen_random_uuid(),
    sequence_id uuid    not null references sequences (id) on delete cascade,
    position    integer not null,
    -- Days to wait after the previous step; 0 means send on launch.
    delay_days  integer not null default 0 check (delay_days >= 0),
    -- Empty on a followup, which replies inside the first step's thread.
    subject     text    not null default '',
    body        text    not null,
    unique (sequence_id, position)
);

create table campaigns (
    id          uuid primary key default gen_random_uuid(),
    org_id      uuid        not null references orgs (id) on delete cascade,
    name        text        not null,
    sequence_id uuid        not null references sequences (id),
    list_id     uuid        not null references lead_lists (id),
    mailbox_id  uuid        not null references mailboxes (id),
    status      text        not null default 'draft'
                    check (status in ('draft', 'running', 'paused', 'finished')),
    started_at  timestamptz,
    created_at  timestamptz not null default now()
);

create index campaigns_org_id_idx on campaigns (org_id);

-- One row per lead per campaign: where that lead has got to in the sequence.
create table campaign_leads (
    id           uuid primary key default gen_random_uuid(),
    campaign_id  uuid        not null references campaigns (id) on delete cascade,
    lead_id      uuid        not null references leads (id) on delete cascade,
    current_step integer     not null default 0,
    next_run_at  timestamptz,
    status       text        not null default 'pending'
                     check (status in ('pending', 'sent', 'replied', 'bounced', 'failed', 'finished')),
    thread_id    text,
    replied_at   timestamptz,
    bounced_at   timestamptz,
    last_error   text,
    unique (campaign_id, lead_id)
);

create index campaign_leads_due_idx on campaign_leads (next_run_at)
    where status = 'pending';

create table messages (
    id                  uuid primary key default gen_random_uuid(),
    campaign_lead_id    uuid        not null references campaign_leads (id) on delete cascade,
    step_id             uuid        not null references sequence_steps (id),
    provider_message_id text        not null,
    thread_id           text        not null,
    -- Kept from day one so SMTP/IMAP reply matching does not need a backfill.
    message_id_header   text,
    subject             text        not null,
    body                text        not null,
    sent_at             timestamptz not null default now()
);

create index messages_campaign_lead_idx on messages (campaign_lead_id);

create table jobs (
    id           uuid primary key default gen_random_uuid(),
    org_id       uuid        not null references orgs (id) on delete cascade,
    kind         text        not null,
    payload      jsonb       not null default '{}'::jsonb,
    scheduled_at timestamptz not null default now(),
    status       text        not null default 'pending'
                     check (status in ('pending', 'running', 'done', 'failed')),
    locked_at    timestamptz,
    locked_by    text,
    attempts     integer     not null default 0,
    last_error   text,
    created_at   timestamptz not null default now()
);

create index jobs_claimable_idx on jobs (scheduled_at) where status = 'pending';

create table suppressions (
    id         uuid primary key default gen_random_uuid(),
    org_id     uuid        not null references orgs (id) on delete cascade,
    email      text        not null,
    reason     text        not null,
    created_at timestamptz not null default now()
);

create unique index suppressions_org_email_key on suppressions (org_id, lower(email));
