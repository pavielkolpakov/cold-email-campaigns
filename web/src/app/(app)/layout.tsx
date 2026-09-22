import Link from "next/link";
import { redirect } from "next/navigation";
import { Mail } from "lucide-react";

import { AppNav } from "@/components/app-nav";
import { SignOutButton } from "@/components/sign-out-button";
import { getCurrentUser } from "@/lib/server-api";

export default async function AppLayout({ children }: { children: React.ReactNode }) {
  const user = await getCurrentUser();
  if (!user) redirect("/login");

  return (
    <div className="flex min-h-svh flex-col">
      <a href="#main-content" className="skip-link">Skip to content</a>
      <header className="border-b bg-card">
        <div className="mx-auto max-w-7xl">
          <div className="flex min-h-18 items-center justify-between gap-4 px-5 sm:px-8">
            <div className="flex items-center gap-5">
              <Link href="/dashboard" className="flex items-center gap-2.5 text-base font-medium tracking-tight">
                <Mail className="size-6" strokeWidth={1.5} aria-hidden="true" />
                Cold Email
              </Link>
              <span className="hidden text-2xl font-light text-border sm:block" aria-hidden="true">/</span>
              <span className="hidden text-sm text-muted-foreground sm:block">Workspace</span>
            </div>
            <div className="flex min-w-0 items-center gap-3">
              <span className="hidden max-w-56 truncate text-sm text-muted-foreground md:block">{user.email}</span>
              <SignOutButton />
            </div>
          </div>
          <AppNav />
        </div>
      </header>
      <main id="main-content" className="workspace mx-auto w-full max-w-7xl flex-1 px-5 py-10 sm:px-8 sm:py-12">{children}</main>
      <footer className="mx-auto flex w-full max-w-7xl items-center justify-between gap-4 px-5 py-6 text-xs text-muted-foreground sm:px-8">
        <span>Cold Email</span>
        <span className="font-mono text-[10px] uppercase tracking-widest">Your mailbox. Your conversations.</span>
      </footer>
    </div>
  );
}
