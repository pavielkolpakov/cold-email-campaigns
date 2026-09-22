"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { apiFetch } from "@/lib/api";
import type { Invite } from "@/lib/types";

export function InviteForm() {
  const router = useRouter();
  const [email, setEmail] = useState("");
  const [link, setLink] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function onSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    setLink(null);
    try {
      const { invite } = await apiFetch<{ invite: Invite }>("/invites", {
        method: "POST",
        body: JSON.stringify({ email, role: "member" }),
      });
      setEmail("");
      setLink(`${window.location.origin}/join/${invite.token}`);
      router.refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Something went wrong");
    }
  }

  return (
    <form onSubmit={onSubmit} className="space-y-2">
      <div className="flex items-start gap-2">
        <Input
          aria-label="Teammate email"
          type="email"
          value={email}
          onChange={(event) => setEmail(event.target.value)}
          placeholder="teammate@company.com"
          required
        />
        <Button type="submit">Invite</Button>
      </div>
      {error && <p className="text-sm text-destructive">{error}</p>}
      {link && (
        <div className="rounded-md border p-3 text-sm">
          <p className="text-muted-foreground">
            Send them this link — nothing is emailed yet:
          </p>
          <code className="mt-1 block break-all">{link}</code>
        </div>
      )}
    </form>
  );
}
