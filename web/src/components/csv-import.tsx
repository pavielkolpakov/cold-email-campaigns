"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { apiFetch } from "@/lib/api";
import type { ImportReport } from "@/lib/types";

type Mapping = {
  email: string;
  first_name: string;
  last_name: string;
  company: string;
};

/** Reads the header row so the user can say which column is which. */
function readHeaders(csv: string): string[] {
  const [firstLine = ""] = csv.split(/\r?\n/);
  return firstLine
    .split(",")
    .map((header) => header.trim().replace(/^"|"$/g, ""))
    .filter(Boolean);
}

/** Pre-selects the obvious columns so the common case is one click. */
function guess(headers: string[], candidates: string[]): string {
  const match = headers.find((header) =>
    candidates.some((candidate) => header.toLowerCase().replace(/[\s_]/g, "") === candidate),
  );
  return match ?? "";
}

export function CsvImport({ listId }: { listId: string }) {
  const router = useRouter();
  const [csv, setCsv] = useState<string | null>(null);
  const [headers, setHeaders] = useState<string[]>([]);
  const [mapping, setMapping] = useState<Mapping>({
    email: "",
    first_name: "",
    last_name: "",
    company: "",
  });
  const [report, setReport] = useState<ImportReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  async function onFile(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    if (!file) return;
    const text = await file.text();
    const found = readHeaders(text);
    setCsv(text);
    setHeaders(found);
    setReport(null);
    setError(null);
    setMapping({
      email: guess(found, ["email", "emailaddress"]),
      first_name: guess(found, ["first", "firstname"]),
      last_name: guess(found, ["last", "lastname"]),
      company: guess(found, ["company", "organization", "account"]),
    });
  }

  async function onImport() {
    if (!csv) return;
    setPending(true);
    setError(null);
    try {
      const result = await apiFetch<{ report: ImportReport }>(`/lead-lists/${listId}/import`, {
        method: "POST",
        body: JSON.stringify({
          csv,
          mapping: {
            email: mapping.email,
            first_name: mapping.first_name || null,
            last_name: mapping.last_name || null,
            company: mapping.company || null,
          },
        }),
      });
      setReport(result.report);
      router.refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Something went wrong");
    } finally {
      setPending(false);
    }
  }

  const fields: { key: keyof Mapping; label: string; required?: boolean }[] = [
    { key: "email", label: "Email", required: true },
    { key: "first_name", label: "First name" },
    { key: "last_name", label: "Last name" },
    { key: "company", label: "Company" },
  ];

  return (
    <div className="space-y-4 rounded-md border p-4">
      <div className="space-y-2">
        <Label htmlFor="csv">Import a CSV</Label>
        <input
          id="csv"
          type="file"
          accept=".csv,text/csv"
          onChange={onFile}
          className="block text-sm file:mr-3 file:rounded-md file:border file:bg-background file:px-3 file:py-1.5"
        />
      </div>

      {headers.length > 0 && (
        <>
          <div className="grid gap-3 sm:grid-cols-2">
            {fields.map((field) => (
              <div key={field.key} className="space-y-1">
                <Label htmlFor={field.key}>
                  {field.label}
                  {field.required && " *"}
                </Label>
                <select
                  id={field.key}
                  value={mapping[field.key]}
                  onChange={(event) =>
                    setMapping((current) => ({ ...current, [field.key]: event.target.value }))
                  }
                  className="h-9 w-full rounded-md border bg-transparent px-3 text-sm"
                >
                  <option value="">— not mapped —</option>
                  {headers.map((header) => (
                    <option key={header} value={header}>
                      {header}
                    </option>
                  ))}
                </select>
              </div>
            ))}
          </div>
          <p className="text-sm text-muted-foreground">
            Unmapped columns are kept and can be used as merge tags.
          </p>
          <Button onClick={onImport} disabled={pending || !mapping.email}>
            {pending ? "Importing…" : "Import"}
          </Button>
        </>
      )}

      {error && <p className="text-sm text-destructive">{error}</p>}

      {report && (
        <div className="space-y-2 text-sm">
          <p>
            Imported {report.imported} · {report.duplicates} already on file ·{" "}
            {report.errors.length} skipped
          </p>
          {report.errors.length > 0 && (
            <ul className="space-y-1 text-muted-foreground">
              {report.errors.slice(0, 10).map((rowError) => (
                <li key={rowError.row}>
                  Row {rowError.row}: {rowError.message}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}
