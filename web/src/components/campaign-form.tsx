"use client";

import Link from "next/link";
import { ArrowRight } from "lucide-react";

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
      <section className="panel overflow-hidden">
        <div className="border-b p-6 sm:p-8">
          <p className="eyebrow mb-3">Before your first send</p>
          <h2 className="text-xl font-medium tracking-tight">A little setup. A better first impression.</h2>
          <p className="mt-2 max-w-xl text-sm leading-6 text-muted-foreground">Every campaign starts with three things: a sending address, the right people, and a message worth opening.</p>
        </div>
        <div className="divide-y">
          {[
            { number: "01", title: "Connect a mailbox", detail: "Send from your own email address.", href: "/mailboxes", done: mailboxes.length > 0 },
            { number: "02", title: "Build a lead list", detail: "Import and organize your contacts.", href: "/leads", done: lists.length > 0 },
            { number: "03", title: "Write a sequence", detail: "Create your message and follow-ups.", href: "/sequences", done: sequences.length > 0 },
          ].map((step) => (
            <Link key={step.number} href={step.href} className="flex items-center gap-4 p-6 transition-colors hover:bg-muted sm:gap-6 sm:px-8">
              <span className="font-mono text-xs text-muted-foreground">{step.number}</span>
              <div className="flex-1"><h3 className="font-medium">{step.title}</h3><p className="mt-1 text-sm text-muted-foreground">{step.detail}</p></div>
              {step.done ? <span className="font-mono text-[11px] text-[#297a3a]">Ready ✓</span> : <ArrowRight className="size-4 text-muted-foreground" aria-hidden="true" />}
            </Link>
          ))}
        </div>
      </section>
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
