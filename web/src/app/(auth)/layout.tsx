import Link from "next/link";
import { Mail } from "lucide-react";

export default function AuthLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex min-h-svh flex-col">
      <header className="border-b bg-card px-6 py-6 sm:px-10">
        <Link href="/" className="inline-flex items-center gap-2.5 text-base font-medium tracking-tight">
          <Mail className="size-6" strokeWidth={1.5} aria-hidden="true" /> Cold Email
        </Link>
      </header>
      <main className="flex flex-1 flex-col items-center justify-center gap-7 px-5 py-12">
        <p className="eyebrow">A workspace for better outreach</p>
        {children}
      </main>
      <footer className="px-6 py-6 text-center font-mono text-[10px] uppercase tracking-widest text-muted-foreground">Your mailbox. Your conversations.</footer>
    </div>
  );
}
