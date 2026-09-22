-- Connected sending mailboxes. OAuth tokens are stored encrypted by the app.

create table mailboxes (
    id                   uuid primary key default gen_random_uuid(),
    org_id               uuid        not null references orgs (id) on delete cascade,
    provider             text        not null check (provider in ('gmail', 'microsoft', 'smtp')),
    email                text        not null,
    access_token_enc     text        not null,
    refresh_token_enc    text        not null,
    token_expires_at     timestamptz not null,
    status               text        not null default 'active'
                             check (status in ('active', 'disconnected')),
    daily_cap            integer     not null default 50,
    created_at           timestamptz not null default now(),
    updated_at           timestamptz not null default now()
);

-- One mailbox per address per org; reconnecting updates the existing row.
create unique index mailboxes_org_email_key on mailboxes (org_id, lower(email));
