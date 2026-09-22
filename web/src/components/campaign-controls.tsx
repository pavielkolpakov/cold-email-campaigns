"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { apiFetch } from "@/lib/api";

export function CampaignControls({ id, status }: { id: string; status: string }) {
  const router = useRouter();
  const [message, setMessage] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  async function act(action: string, describe: (result: unknown) => string) {
    setPending(true);
    setMessage(null);
    try {
      const result = await apiFetch(`/campaigns/${id}/${action}`, { method: "POST" });
      setMessage(describe(result));
      router.refresh();
    } catch (err) {
      setMessage(err instanceof Error ? err.message : "Something went wrong");
    } finally {
      setPending(false);
    }
  }

  return (
    <div className="flex items-center gap-2">
      {status === "draft" && (
        <Button
          disabled={pending}
          onClick={() =>
            act("launch", (result) => {
              const enrolled = (result as { enrolled: number }).enrolled;
              return `${enrolled} ${enrolled === 1 ? "lead" : "leads"} enrolled.`;
            })
          }
        >
          Launch
        </Button>
      )}
      {status === "running" && (
        <Button variant="outline" disabled={pending} onClick={() => act("pause", () => "Paused.")}>
          Pause
        </Button>
      )}
      {status === "paused" && (
        <Button disabled={pending} onClick={() => act("resume", () => "Resumed.")}>
          Resume
        </Button>
      )}
      {message && <span className="text-sm text-muted-foreground">{message}</span>}
    </div>
  );
}
