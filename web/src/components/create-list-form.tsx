"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { apiFetch } from "@/lib/api";

export function CreateListForm() {
  const router = useRouter();
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);

  async function onSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    try {
      await apiFetch("/lead-lists", { method: "POST", body: JSON.stringify({ name }) });
      setName("");
      router.refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Something went wrong");
    }
  }

  return (
    <form onSubmit={onSubmit} className="flex items-start gap-2">
      <div className="space-y-1">
        <Input
          aria-label="List name"
          value={name}
          onChange={(event) => setName(event.target.value)}
          placeholder="New list name"
          required
        />
        {error && <p className="text-sm text-destructive">{error}</p>}
      </div>
      <Button type="submit">Create list</Button>
    </form>
  );
}
