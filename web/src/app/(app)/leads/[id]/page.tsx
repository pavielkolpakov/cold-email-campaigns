import { notFound } from "next/navigation";

import { CsvImport } from "@/components/csv-import";
import { serverFetch } from "@/lib/server-api";
import type { Lead, LeadList } from "@/lib/types";

export default async function LeadListPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const [lists, leadsData] = await Promise.all([
    serverFetch<{ lists: LeadList[] }>("/lead-lists"),
    serverFetch<{ leads: Lead[] }>(`/lead-lists/${id}/leads`),
  ]);

  const list = lists?.lists.find((candidate) => candidate.id === id);
  if (!list) notFound();

  const leads = leadsData?.leads ?? [];

  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-semibold">{list.name}</h1>
      <CsvImport listId={id} />

      <p className="text-sm text-muted-foreground">
        {leads.length} {leads.length === 1 ? "lead" : "leads"}
      </p>

      {leads.length > 0 && (
        <table className="w-full text-left text-sm">
          <thead className="border-b text-muted-foreground">
            <tr>
              <th className="py-2 font-medium">Email</th>
              <th className="py-2 font-medium">Name</th>
              <th className="py-2 font-medium">Company</th>
              <th className="py-2 font-medium">Status</th>
            </tr>
          </thead>
          <tbody className="divide-y">
            {leads.map((lead) => (
              <tr key={lead.id}>
                <td className="py-2">{lead.email}</td>
                <td className="py-2">
                  {[lead.first_name, lead.last_name].filter(Boolean).join(" ") || "—"}
                </td>
                <td className="py-2">{lead.company ?? "—"}</td>
                <td className="py-2">{lead.status}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
