-- Invitations to join an existing org. The token is the whole secret, so it is
-- single-use and expires.

create table invites (
    id         uuid primary key default gen_random_uuid(),
    org_id     uuid        not null references orgs (id) on delete cascade,
    email      text        not null,
    role       text        not null default 'member' check (role in ('owner', 'member')),
    token      uuid        not null default gen_random_uuid(),
    invited_by uuid        not null references users (id) on delete cascade,
    accepted_at timestamptz,
    expires_at timestamptz not null default now() + interval '7 days',
    created_at timestamptz not null default now()
);

create unique index invites_token_key on invites (token);
create index invites_org_id_idx on invites (org_id);
