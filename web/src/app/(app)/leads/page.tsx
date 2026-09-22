import Link from "next/link";

import { CreateListForm } from "@/components/create-list-form";
import { serverFetch } from "@/lib/server-api";
import type { LeadList } from "@/lib/types";

export default async function LeadsPage() {
  const data = await serverFetch<{ lists: LeadList[] }>("/lead-lists");
  const lists = data?.lists ?? [];

  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-semibold">Lead lists</h1>
      <CreateListForm />

      {lists.length === 0 ? (
        <p className="text-muted-foreground">No lists yet. Create one to import leads into.</p>
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
