import Link from "next/link";
import { ArrowRight, Inbox, Mail, Plus } from "lucide-react";

import { DisconnectedBanner } from "@/components/disconnected-banner";
import { buttonVariants } from "@/components/ui/button";
import { serverFetch } from "@/lib/server-api";
import type { DashboardSummary } from "@/lib/types";

export default async function DashboardPage() {
  const data = await serverFetch<{ summary: DashboardSummary }>("/dashboard");
  const summary = data?.summary;

  if (!summary) {
    return <div className="empty-state"><h1>Overview unavailable</h1><p>Could not load the dashboard. Refresh the page to try again.</p></div>;
  }

  const sentToday = summary.mailboxes.reduce((total, mailbox) => total + mailbox.sent_today, 0);
  const dailyCapacity = summary.mailboxes.reduce((total, mailbox) => total + mailbox.daily_cap, 0);

  return (
    <div className="space-y-8">
      <div className="page-heading">
        <div><h1>Overview</h1><p>Your outreach, at a glance. Keep conversations moving.</p></div>
        <Link href="/campaigns" className={buttonVariants()}><Plus aria-hidden="true" /> Create campaign</Link>
      </div>
      <DisconnectedBanner mailboxes={summary.mailboxes} />

      <section aria-labelledby="today-heading">
        <h2 id="today-heading" className="eyebrow mb-3">Today’s activity</h2>
        <div className="panel grid divide-y sm:grid-cols-3 sm:divide-x sm:divide-y-0">
          {[
            { label: "Active campaigns", value: summary.active_campaigns, note: "Currently running" },
            { label: "Emails sent", value: sentToday, note: `${dailyCapacity} daily sending capacity` },
            { label: "Connected mailboxes", value: summary.mailboxes.filter((m) => m.status === "active").length, note: `${summary.mailboxes.length} ${summary.mailboxes.length === 1 ? "mailbox" : "mailboxes"} in your workspace` },
          ].map((metric) => (
            <div key={metric.label} className="p-6">
              <p className="text-sm text-muted-foreground">{metric.label}</p>
              <p className="my-4 text-4xl font-normal tracking-tighter tabular-nums">{metric.value}</p>
              <p className="text-xs text-muted-foreground">{metric.note}</p>
            </div>
          ))}
        </div>
      </section>

      {summary.problems.length > 0 && (
        <section className="space-y-3">
          <h2 className="eyebrow">Needs attention · {summary.problems.length}</h2>
          <ul className="divide-y rounded-md border">
            {summary.problems.map((problem) => (
              <li key={`${problem.campaign_id}-${problem.lead_email}`} className="p-4 text-sm">
                <Link href={`/campaigns/${problem.campaign_id}`} className="font-medium hover:underline">{problem.campaign_name}</Link>
                <span className="text-muted-foreground"> · {problem.lead_email}</span>
                <p className="mt-1 text-muted-foreground">{problem.error}</p>
              </li>
            ))}
          </ul>
        </section>
      )}

      <div className="grid items-start gap-8 lg:grid-cols-[1.4fr_1fr]">
        <section className="space-y-4">
          <div className="flex min-h-6 items-center justify-between"><h2 className="text-base font-medium">Recent replies</h2><span className="eyebrow">Inbox activity</span></div>
          {summary.recent_replies.length === 0 ? (
            <div className="empty-state min-h-64 bg-card">
              <Inbox aria-hidden="true" /><h3 className="font-medium">The next conversation starts here</h3>
              <p>Replies to your campaigns will appear here as your outreach gets underway.</p>
              <Link href="/campaigns" className="mt-2 inline-flex items-center gap-2 text-sm hover:underline">View campaigns <ArrowRight className="size-4" aria-hidden="true" /></Link>
            </div>
          ) : (
            <ul className="divide-y rounded-md border">
              {summary.recent_replies.map((reply) => (
                <li key={`${reply.lead_email}-${reply.replied_at}`} className="flex justify-between p-4 text-sm">
                  <span className="font-medium">{reply.lead_email}</span><span className="text-muted-foreground">{reply.campaign_name}</span>
                </li>
              ))}
            </ul>
          )}
        </section>
        <section className="space-y-4">
          <div className="flex min-h-6 items-center justify-between"><h2 className="text-base font-medium">Sending capacity</h2><Link href="/mailboxes" className="text-xs text-muted-foreground hover:text-foreground">Manage mailboxes <span aria-hidden="true">↗</span></Link></div>
          {summary.mailboxes.length === 0 ? (
            <div className="empty-state min-h-64 bg-card"><Mail aria-hidden="true" /><h3 className="font-medium">Connect your first mailbox</h3><p>Send campaigns from an address your contacts can reply to.</p><Link href="/mailboxes" className={buttonVariants({ variant: "outline" })}>Set up a mailbox</Link></div>
          ) : (
            <div className="panel divide-y">
              {summary.mailboxes.map((mailbox) => {
                const pct = mailbox.daily_cap > 0 ? Math.min(100, Math.round((mailbox.sent_today / mailbox.daily_cap) * 100)) : 0;
                return (
                  <div key={mailbox.id} className="space-y-4 p-5">
                    <div className="flex items-center gap-3"><Mail className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" /><span className="truncate text-sm font-medium">{mailbox.email}</span></div>
                    <div className="flex justify-between gap-3 text-xs text-muted-foreground"><span className="capitalize">{mailbox.status}</span><span className="font-mono tabular-nums">{mailbox.sent_today} / {mailbox.daily_cap} sent</span></div>
                    <div role="progressbar" aria-label={`${mailbox.email} daily sending capacity used`} aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100} className="h-1 overflow-hidden rounded-full bg-muted"><div className="h-full bg-primary" style={{ width: `${pct}%` }} /></div>
                  </div>
                );
              })}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
