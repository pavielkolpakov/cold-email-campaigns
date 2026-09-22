import "server-only";

import { cookies } from "next/headers";

import type { User } from "@/lib/api";

const API_URL = process.env.API_URL ?? "http://localhost:8080";

/** Server-side call; forwards the caller's session cookie to the Rust API. */
export async function serverFetch<T>(path: string): Promise<T | null> {
  const cookieHeader = (await cookies()).toString();
  const res = await fetch(`${API_URL}${path}`, {
    headers: { cookie: cookieHeader },
    cache: "no-store",
  });
  if (!res.ok) return null;
  return (await res.json()) as T;
}

export function getCurrentUser() {
  return serverFetch<User>("/auth/me");
}
