import { getCurrentUser } from "@/lib/server-api";

export default async function DashboardPage() {
  const user = await getCurrentUser();

  return (
    <div className="space-y-2">
      <h1 className="text-2xl font-semibold">Dashboard</h1>
      <p className="text-muted-foreground">
        Signed in as {user?.name}. Connect a mailbox to get started.
      </p>
    </div>
  );
}
