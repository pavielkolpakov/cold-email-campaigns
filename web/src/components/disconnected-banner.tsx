import type { Mailbox } from "@/lib/types";

/// A disconnected mailbox stops every campaign using it, so it is stated at the
/// top of the page with the one action that fixes it.
export function DisconnectedBanner({ mailboxes }: { mailboxes: Pick<Mailbox, "email" | "status">[] }) {
  const broken = mailboxes.filter((mailbox) => mailbox.status !== "active");
  if (broken.length === 0) return null;

  return (
    <div className="rounded-md border border-destructive/50 bg-destructive/5 p-4">
      <p className="font-medium text-destructive">
        {broken.length === 1
          ? `${broken[0].email} is disconnected`
          : `${broken.length} mailboxes are disconnected`}
      </p>
      <p className="mt-1 text-sm text-muted-foreground">
        Google revoked or expired the access. Campaigns using{" "}
        {broken.length === 1 ? "it" : "them"} cannot send until reconnected.
      </p>
      <a
        href="/api/mailboxes/gmail/authorize"
        className="mt-3 inline-flex h-9 items-center rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground hover:bg-primary/90"
      >
        Reconnect
      </a>
    </div>
  );
}
