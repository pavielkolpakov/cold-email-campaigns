import { InviteForm } from "@/components/invite-form";
import { getCurrentUser, serverFetch } from "@/lib/server-api";
import type { Invite } from "@/lib/types";

export default async function TeamPage() {
  const user = await getCurrentUser();
  const data = await serverFetch<{ invites: Invite[] }>("/invites");

  if (user?.role !== "owner") {
    return (
      <div className="space-y-2">
        <div className="page-heading"><div><h1>Team</h1><p>Manage the people who share your workspace.</p></div></div>
        <p className="text-muted-foreground">Only an owner can manage invitations.</p>
      </div>
    );
  }

  const invites = data?.invites ?? [];

  return (
    <div className="space-y-8">
      <div className="page-heading"><div><h1>Team</h1><p>Manage the people who share your workspace.</p></div></div>
      <InviteForm />

      {invites.length > 0 && (
        <ul className="divide-y rounded-md border">
          {invites.map((invite) => (
            <li key={invite.id} className="flex items-center justify-between p-3 text-sm">
              <span className="font-medium">{invite.email}</span>
              <span className="text-muted-foreground">
                {invite.accepted_at ? "joined" : "invited"} · {invite.role}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
