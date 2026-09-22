"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { apiFetch } from "@/lib/api";
import type { Mailbox } from "@/lib/types";

export function MailboxList({ mailboxes }: { mailboxes: Mailbox[] }) {
  const router = useRouter();
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  async function run(id: string, action: () => Promise<unknown>, success: string) {
    setBusy(id);
    setMessage(null);
    try {
      await action();
      setMessage(success);
      router.refresh();
    } catch (err) {
      setMessage(err instanceof Error ? err.message : "Something went wrong");
    } finally {
      setBusy(null);
    }
  }

  if (mailboxes.length === 0) {
    return <p className="text-muted-foreground">No mailboxes connected yet.</p>;
  }

  return (
    <div className="space-y-3">
      {message && <p className="text-sm">{message}</p>}
      <ul className="divide-y rounded-md border">
        {mailboxes.map((mailbox) => (
          <li key={mailbox.id} className="flex items-center justify-between p-4">
            <div>
              <p className="font-medium">{mailbox.email}</p>
              <p className="text-sm text-muted-foreground">
                {mailbox.provider} · {mailbox.status} · {mailbox.daily_cap}/day
              </p>
            </div>
            <div className="flex gap-2">
              <Button
                variant="outline"
                size="sm"
                disabled={busy === mailbox.id}
                onClick={() =>
                  run(
                    mailbox.id,
                    () => apiFetch(`/mailboxes/${mailbox.id}/test`, { method: "POST" }),
                    `Test email sent to ${mailbox.email}.`,
                  )
                }
              >
                Send test
              </Button>
              <Button
                variant="ghost"
                size="sm"
                disabled={busy === mailbox.id}
                onClick={() =>
                  run(
                    mailbox.id,
                    () => apiFetch(`/mailboxes/${mailbox.id}`, { method: "DELETE" }),
                    "Mailbox disconnected.",
                  )
                }
              >
                Disconnect
              </Button>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
