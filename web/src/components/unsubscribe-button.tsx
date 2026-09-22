"use client";

import { useState } from "react";

import { Button } from "@/components/ui/button";
import { apiFetch } from "@/lib/api";

export function UnsubscribeButton({ token }: { token: string }) {
  const [state, setState] = useState<"idle" | "done" | "error">("idle");

  if (state === "done") {
    return <p className="text-sm">You have been unsubscribed. You will not hear from us again.</p>;
  }

  return (
    <div className="space-y-2">
      <Button
        onClick={async () => {
          try {
            await apiFetch(`/unsubscribe/${token}`, { method: "POST" });
            setState("done");
          } catch {
            setState("error");
          }
        }}
      >
        Unsubscribe
      </Button>
      {state === "error" && (
        <p className="text-sm text-destructive">That link is no longer valid.</p>
      )}
    </div>
  );
}
