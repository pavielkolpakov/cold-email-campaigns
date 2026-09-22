import Link from "next/link";

import { CampaignForm } from "@/components/campaign-form";
import { serverFetch } from "@/lib/server-api";
import type { Campaign, LeadList, Mailbox, Sequence } from "@/lib/types";

export default async function CampaignsPage() {
  const [campaigns, sequences, lists, mailboxes] = await Promise.all([
    serverFetch<{ campaigns: Campaign[] }>("/campaigns"),
    serverFetch<{ sequences: Sequence[] }>("/sequences"),
    serverFetch<{ lists: LeadList[] }>("/lead-lists"),
    serverFetch<{ mailboxes: Mailbox[] }>("/mailboxes"),
  ]);

  return (
    <div className="space-y-8">
      <div className="page-heading"><div><h1>Campaigns</h1><p>Bring your audience, sequence, and mailbox together.</p></div></div>

      <CampaignForm
        sequences={sequences?.sequences ?? []}
        lists={lists?.lists ?? []}
        mailboxes={(mailboxes?.mailboxes ?? []).filter((m) => m.status === "active")}
      />

      {(campaigns?.campaigns ?? []).length > 0 && (
        <ul className="divide-y rounded-md border">
          {campaigns!.campaigns.map((campaign) => (
            <li key={campaign.id} className="flex items-center justify-between p-4">
              <Link href={`/campaigns/${campaign.id}`} className="font-medium hover:underline">
                {campaign.name}
              </Link>
              <span className="text-sm text-muted-foreground">{campaign.status}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
