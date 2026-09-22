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
