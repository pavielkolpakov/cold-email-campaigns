-- Core tenancy: orgs, their users, and login sessions.

create extension if not exists "pgcrypto";

create table orgs (
    id         uuid primary key default gen_random_uuid(),
    name       text        not null,
    created_at timestamptz not null default now()
);

create table users (
    id            uuid primary key default gen_random_uuid(),
    org_id        uuid        not null references orgs (id) on delete cascade,
    email         text        not null,
    password_hash text        not null,
    name          text        not null,
    role          text        not null default 'member' check (role in ('owner', 'member')),
    created_at    timestamptz not null default now()
);

-- Email is globally unique: a person belongs to exactly one org for now.
create unique index users_email_key on users (lower(email));
create index users_org_id_idx on users (org_id);

create table sessions (
    id         uuid primary key default gen_random_uuid(),
    user_id    uuid        not null references users (id) on delete cascade,
    expires_at timestamptz not null,
    created_at timestamptz not null default now()
);

create index sessions_user_id_idx on sessions (user_id);
create index sessions_expires_at_idx on sessions (expires_at);
