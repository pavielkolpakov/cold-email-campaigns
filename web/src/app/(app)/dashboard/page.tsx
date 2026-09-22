import Link from "next/link";

import { DisconnectedBanner } from "@/components/disconnected-banner";
import { serverFetch } from "@/lib/server-api";
import type { DashboardSummary } from "@/lib/types";

export default async function DashboardPage() {
  const data = await serverFetch<{ summary: DashboardSummary }>("/dashboard");
  const summary = data?.summary;

  if (!summary) {
    return <p className="text-muted-foreground">Could not load the dashboard.</p>;
  }

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-semibold">Dashboard</h1>

      <DisconnectedBanner mailboxes={summary.mailboxes} />

      <section className="space-y-2">
        <h2 className="text-sm font-medium text-muted-foreground">Today</h2>
        <div className="grid gap-3 sm:grid-cols-3">
          <div className="rounded-md border p-4">
            <p className="text-sm text-muted-foreground">Active campaigns</p>
            <p className="text-2xl font-semibold tabular-nums">{summary.active_campaigns}</p>
          </div>
          {summary.mailboxes.map((mailbox) => {
            const pct = Math.min(100, Math.round((mailbox.sent_today / mailbox.daily_cap) * 100));
            return (
              <div key={mailbox.id} className="rounded-md border p-4">
                <p className="truncate text-sm text-muted-foreground">{mailbox.email}</p>
                <p className="text-2xl font-semibold tabular-nums">
                  {mailbox.sent_today}
                  <span className="text-base font-normal text-muted-foreground">
                    {" "}
                    / {mailbox.daily_cap}
                  </span>
                </p>
                <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-muted">
                  <div className="h-full bg-primary" style={{ width: `${pct}%` }} />
                </div>
              </div>
            );
          })}
        </div>
      </section>

      {summary.problems.length > 0 && (
        <section className="space-y-2">
          <h2 className="text-sm font-medium text-muted-foreground">Needs attention</h2>
          <ul className="divide-y rounded-md border">
            {summary.problems.map((problem) => (
              <li key={`${problem.campaign_id}-${problem.lead_email}`} className="p-3 text-sm">
                <Link href={`/campaigns/${problem.campaign_id}`} className="font-medium hover:underline">
                  {problem.campaign_name}
                </Link>
                <span className="text-muted-foreground"> · {problem.lead_email}</span>
                <p className="text-muted-foreground">{problem.error}</p>
              </li>
            ))}
          </ul>
        </section>
      )}

      <section className="space-y-2">
        <h2 className="text-sm font-medium text-muted-foreground">Recent replies</h2>
        {summary.recent_replies.length === 0 ? (
          <p className="text-sm text-muted-foreground">No replies yet.</p>
        ) : (
          <ul className="divide-y rounded-md border">
            {summary.recent_replies.map((reply) => (
              <li key={`${reply.lead_email}-${reply.replied_at}`} className="flex justify-between p-3 text-sm">
                <span className="font-medium">{reply.lead_email}</span>
                <span className="text-muted-foreground">{reply.campaign_name}</span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
