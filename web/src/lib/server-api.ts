import "server-only";

import { cookies } from "next/headers";

import type { User } from "@/lib/api";

const API_URL = process.env.API_URL ?? "http://localhost:8080";

/** Server-side call; forwards the caller's session cookie to the Rust API. */
export async function serverFetch<T>(path: string): Promise<T | null> {
  const cookieHeader = (await cookies()).toString();

  let res: Response;
  try {
    res = await fetch(`${API_URL}${path}`, {
      headers: { cookie: cookieHeader },
      cache: "no-store",
    });
  } catch (cause) {
    // A refused connection otherwise surfaces as a blank Server Components
    // error with no message, which says nothing about the API being down.
    throw new Error(
      `Cannot reach the API at ${API_URL}. Is the Rust service running? (cargo run -- api)`,
      { cause },
    );
  }

  // A 4xx is an answer — unauthorized, not found — and the caller decides.
  if (!res.ok) return null;
  return (await res.json()) as T;
}

export function getCurrentUser() {
  return serverFetch<User>("/auth/me");
}
