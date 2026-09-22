"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { apiFetch } from "@/lib/api";
import type { Campaign, LeadList, Mailbox, Sequence } from "@/lib/types";

export function CampaignForm({
  sequences,
  lists,
  mailboxes,
}: {
  sequences: Sequence[];
  lists: LeadList[];
  mailboxes: Mailbox[];
}) {
  const router = useRouter();
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  const ready = sequences.length > 0 && lists.length > 0 && mailboxes.length > 0;

  async function onSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    setPending(true);
    const form = new FormData(event.currentTarget);
    try {
      const { campaign } = await apiFetch<{ campaign: Campaign }>("/campaigns", {
        method: "POST",
        body: JSON.stringify({
          name: form.get("name"),
          sequence_id: form.get("sequence_id"),
          list_id: form.get("list_id"),
          mailbox_id: form.get("mailbox_id"),
        }),
      });
      router.push(`/campaigns/${campaign.id}`);
      router.refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Something went wrong");
      setPending(false);
    }
  }

  if (!ready) {
    return (
      <p className="text-muted-foreground">
        A campaign needs a sequence, a lead list and a connected mailbox first.
      </p>
    );
  }

  const selects = [
    { name: "sequence_id", label: "Sequence", options: sequences.map((s) => [s.id, s.name]) },
    { name: "list_id", label: "Lead list", options: lists.map((l) => [l.id, l.name]) },
    { name: "mailbox_id", label: "Send from", options: mailboxes.map((m) => [m.id, m.email]) },
  ] as const;

  return (
    <form onSubmit={onSubmit} className="space-y-4 rounded-md border p-4">
      <div className="space-y-2">
        <Label htmlFor="name">Campaign name</Label>
        <Input id="name" name="name" required />
      </div>

      {selects.map((select) => (
        <div key={select.name} className="space-y-2">
          <Label htmlFor={select.name}>{select.label}</Label>
          <select
            id={select.name}
            name={select.name}
            required
            className="h-9 w-full rounded-md border bg-transparent px-3 text-sm"
          >
            {select.options.map(([value, label]) => (
              <option key={value} value={value}>
                {label}
              </option>
            ))}
          </select>
        </div>
      ))}

      {error && <p className="text-sm text-destructive">{error}</p>}

      <Button type="submit" disabled={pending}>
        {pending ? "Creating…" : "Create campaign"}
      </Button>
    </form>
  );
}
