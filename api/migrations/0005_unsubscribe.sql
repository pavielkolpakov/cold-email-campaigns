-- Every lead carries an unguessable opt-out token, so the unsubscribe link in a
-- sent email keeps working without the recipient ever signing in.

alter table leads
    add column unsubscribe_token uuid not null default gen_random_uuid();

create unique index leads_unsubscribe_token_key on leads (unsubscribe_token);
