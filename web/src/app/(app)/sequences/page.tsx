import { SequenceForm } from "@/components/sequence-form";
import { serverFetch } from "@/lib/server-api";
import type { Sequence } from "@/lib/types";

export default async function SequencesPage() {
  const data = await serverFetch<{ sequences: Sequence[] }>("/sequences");
  const sequences = data?.sequences ?? [];

  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-semibold">Sequences</h1>
      <SequenceForm />

      {sequences.length > 0 && (
        <ul className="divide-y rounded-md border">
          {sequences.map((sequence) => (
            <li key={sequence.id} className="p-4 font-medium">
              {sequence.name}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
