"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { apiFetch } from "@/lib/api";

type Draft = { delay_days: number; subject: string; body: string };

const FIRST_STEP: Draft = { delay_days: 0, subject: "", body: "" };

export function SequenceForm() {
  const router = useRouter();
  const [name, setName] = useState("");
  const [steps, setSteps] = useState<Draft[]>([{ ...FIRST_STEP }]);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  function update(index: number, patch: Partial<Draft>) {
    setSteps((current) =>
      current.map((step, position) => (position === index ? { ...step, ...patch } : step)),
    );
  }

  async function onSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    setPending(true);
    try {
      await apiFetch("/sequences", {
        method: "POST",
        body: JSON.stringify({ name, steps }),
      });
      setName("");
      setSteps([{ ...FIRST_STEP }]);
      router.refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Something went wrong");
    } finally {
      setPending(false);
    }
  }

  return (
    <form onSubmit={onSubmit} className="space-y-4 rounded-md border p-4">
      <div className="space-y-2">
        <Label htmlFor="name">Sequence name</Label>
        <Input id="name" value={name} onChange={(e) => setName(e.target.value)} required />
      </div>

      {steps.map((step, index) => (
        <div key={index} className="space-y-2 rounded-md border p-3">
          <div className="flex items-center justify-between">
            <p className="font-medium">Step {index + 1}</p>
            {index > 0 && (
              <div className="flex items-center gap-2 text-sm">
                <Label htmlFor={`delay-${index}`}>Wait</Label>
                <Input
                  id={`delay-${index}`}
                  type="number"
                  min={0}
                  value={step.delay_days}
                  onChange={(e) => update(index, { delay_days: Number(e.target.value) })}
                  className="w-20"
                />
                <span className="text-muted-foreground">days</span>
              </div>
            )}
          </div>

          {index === 0 ? (
            <div className="space-y-1">
              <Label htmlFor={`subject-${index}`}>Subject</Label>
              <Input
                id={`subject-${index}`}
                value={step.subject}
                onChange={(e) => update(index, { subject: e.target.value })}
                required
              />
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">Replies inside the first step&apos;s thread.</p>
          )}

          <div className="space-y-1">
            <Label htmlFor={`body-${index}`}>Body</Label>
            <textarea
              id={`body-${index}`}
              value={step.body}
              onChange={(e) => update(index, { body: e.target.value })}
              rows={4}
              required
              className="w-full rounded-md border bg-transparent p-2 text-sm"
            />
          </div>
        </div>
      ))}

      <p className="text-sm text-muted-foreground">
        Merge tags: <code>{"{{first_name}}"}</code>, <code>{"{{company}}"}</code>, any CSV column.
        Give every tag a fallback like <code>{"{{first_name|there}}"}</code> — a lead with no value
        and no fallback is skipped rather than mailed a gap.
      </p>

      {error && <p className="text-sm text-destructive">{error}</p>}

      <div className="flex gap-2">
        <Button
          type="button"
          variant="outline"
          onClick={() => setSteps((current) => [...current, { delay_days: 3, subject: "", body: "" }])}
        >
          Add followup
        </Button>
        <Button type="submit" disabled={pending}>
          {pending ? "Saving…" : "Save sequence"}
        </Button>
      </div>
    </form>
  );
}
