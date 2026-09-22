import { notFound } from "next/navigation";

import { CampaignControls } from "@/components/campaign-controls";
import { serverFetch } from "@/lib/server-api";
import type { Campaign, CampaignStats } from "@/lib/types";

export default async function CampaignPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const data = await serverFetch<{ campaign: Campaign; stats: CampaignStats }>(`/campaigns/${id}`);
  if (!data) notFound();

  const { campaign, stats } = data;
  const tiles: [string, number][] = [
    ["Total", stats.total],
    ["Pending", stats.pending],
    ["Sent", stats.sent],
    ["Replied", stats.replied],
    ["Bounced", stats.bounced],
    ["Failed", stats.failed],
  ];

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold">{campaign.name}</h1>
          <p className="text-sm text-muted-foreground">{campaign.status}</p>
        </div>
        <CampaignControls id={campaign.id} status={campaign.status} />
      </div>

      <dl className="grid grid-cols-2 gap-3 sm:grid-cols-6">
        {tiles.map(([label, value]) => (
          <div key={label} className="rounded-md border p-3">
            <dt className="text-sm text-muted-foreground">{label}</dt>
            <dd className="text-2xl font-semibold tabular-nums">{value}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}
