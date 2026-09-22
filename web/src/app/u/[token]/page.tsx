import { UnsubscribeButton } from "@/components/unsubscribe-button";

/// Deliberately a button, not an automatic opt-out on page load: mail scanners
/// and link prefetchers follow URLs in emails, and would unsubscribe people who
/// never clicked.
export default async function UnsubscribePage({
  params,
}: {
  params: Promise<{ token: string }>;
}) {
  const { token } = await params;

  return (
    <div className="flex min-h-svh items-center justify-center p-6">
      <div className="w-full max-w-sm space-y-4 rounded-md border p-6">
        <h1 className="text-xl font-semibold">Unsubscribe</h1>
        <p className="text-sm text-muted-foreground">
          Confirm and we will stop emailing this address.
        </p>
        <UnsubscribeButton token={token} />
      </div>
    </div>
  );
}
