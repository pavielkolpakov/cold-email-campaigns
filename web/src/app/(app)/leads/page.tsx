import Link from "next/link";

import { CreateListForm } from "@/components/create-list-form";
import { serverFetch } from "@/lib/server-api";
import type { LeadList } from "@/lib/types";

export default async function LeadsPage() {
  const data = await serverFetch<{ lists: LeadList[] }>("/lead-lists");
  const lists = data?.lists ?? [];

  return (
    <div className="space-y-8">
      <div className="page-heading"><div><h1>Lead lists</h1><p>Organize the people you want to start a conversation with.</p></div></div>
      <CreateListForm />

      {lists.length === 0 ? (
        <div className="empty-state bg-card"><h2 className="font-medium">Make room for your next conversation</h2><p>Create your first list above, then import your contacts from a CSV file.</p></div>
      ) : (
        <ul className="divide-y rounded-md border">
          {lists.map((list) => (
            <li key={list.id} className="p-4">
              <Link href={`/leads/${list.id}`} className="font-medium hover:underline">
                {list.name}
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
