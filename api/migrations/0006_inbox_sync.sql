-- Where each mailbox's inbox sync got to, so a sync resumes rather than
-- re-reading the whole mailbox.

alter table mailboxes add column sync_cursor text;
alter table mailboxes add column last_synced_at timestamptz;

-- Which inbound message stopped a lead, for when someone asks why.
alter table campaign_leads add column reply_message_id text;
