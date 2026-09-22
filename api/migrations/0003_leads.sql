-- Imported prospects, grouped into named lists.

create table lead_lists (
    id         uuid primary key default gen_random_uuid(),
    org_id     uuid        not null references orgs (id) on delete cascade,
    name       text        not null,
    created_at timestamptz not null default now()
);

create index lead_lists_org_id_idx on lead_lists (org_id);

create table leads (
    id         uuid primary key default gen_random_uuid(),
    org_id     uuid        not null references orgs (id) on delete cascade,
    list_id    uuid        not null references lead_lists (id) on delete cascade,
    email      text        not null,
    first_name text,
    last_name  text,
    company    text,
    -- Every CSV column we were not told to map, kept for merge tags.
    custom     jsonb       not null default '{}'::jsonb,
    status     text        not null default 'active'
                   check (status in ('active', 'replied', 'bounced', 'unsubscribed')),
    created_at timestamptz not null default now()
);

-- A prospect appears at most once per org, however many times they are imported.
create unique index leads_org_email_key on leads (org_id, lower(email));
create index leads_list_id_idx on leads (list_id);
