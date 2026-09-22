import { MailboxList } from "@/components/mailbox-list";
import { serverFetch } from "@/lib/server-api";
import type { Mailbox } from "@/lib/types";

export default async function MailboxesPage({
  searchParams,
}: {
  searchParams: Promise<{ connected?: string; error?: string }>;
}) {
  const { connected, error } = await searchParams;
  const data = await serverFetch<{ mailboxes: Mailbox[] }>("/mailboxes");

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold">Mailboxes</h1>
        <a
          href="/api/mailboxes/gmail/authorize"
          className="inline-flex h-9 items-center rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground hover:bg-primary/90"
        >
          Connect Gmail
        </a>
      </div>

      {connected && <p className="text-sm">Mailbox connected.</p>}
      {error && <p className="text-sm text-destructive">Connection was cancelled.</p>}

      <MailboxList mailboxes={data?.mailboxes ?? []} />
    </div>
  );
}
