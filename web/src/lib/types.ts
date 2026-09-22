export type Mailbox = {
  id: string;
  provider: string;
  email: string;
  status: string;
  daily_cap: number;
};

export type LeadList = {
  id: string;
  name: string;
};

export type Lead = {
  id: string;
  email: string;
  first_name: string | null;
  last_name: string | null;
  company: string | null;
  custom: Record<string, string>;
  status: string;
};

export type ImportReport = {
  imported: number;
  duplicates: number;
  errors: { row: number; message: string }[];
};

export type Sequence = {
  id: string;
  name: string;
};

export type Step = {
  id: string;
  position: number;
  delay_days: number;
  subject: string;
  body: string;
};

export type Campaign = {
  id: string;
  name: string;
  sequence_id: string;
  list_id: string;
  mailbox_id: string;
  status: string;
};

export type CampaignStats = {
  total: number;
  pending: number;
  sent: number;
  replied: number;
  bounced: number;
  failed: number;
};

export type DashboardSummary = {
  active_campaigns: number;
  mailboxes: {
    id: string;
    email: string;
    status: string;
    daily_cap: number;
    sent_today: number;
  }[];
  recent_replies: { lead_email: string; campaign_name: string; replied_at: string }[];
  problems: {
    campaign_id: string;
    campaign_name: string;
    lead_email: string;
    error: string;
  }[];
};

export type Invite = {
  id: string;
  email: string;
  role: string;
  token: string;
  accepted_at: string | null;
};
