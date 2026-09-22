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
