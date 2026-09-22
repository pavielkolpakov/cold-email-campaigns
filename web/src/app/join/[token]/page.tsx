import { AcceptInviteForm } from "@/components/accept-invite-form";

export default async function JoinPage({ params }: { params: Promise<{ token: string }> }) {
  const { token } = await params;

  return (
    <div className="flex min-h-svh items-center justify-center bg-muted/40 p-6">
      <div className="w-full max-w-sm space-y-4 rounded-md border bg-background p-6">
        <div>
          <h1 className="text-xl font-semibold">Join the workspace</h1>
          <p className="text-sm text-muted-foreground">
            You were invited. Pick a password to finish.
          </p>
        </div>
        <AcceptInviteForm token={token} />
      </div>
    </div>
  );
}
