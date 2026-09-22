"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

const nav = [
  { href: "/dashboard", label: "Overview" },
  { href: "/campaigns", label: "Campaigns" },
  { href: "/leads", label: "Leads" },
  { href: "/sequences", label: "Sequences" },
  { href: "/mailboxes", label: "Mailboxes" },
  { href: "/team", label: "Team" },
];

export function AppNav() {
  const pathname = usePathname();

  return (
    <nav aria-label="Main navigation" className="flex gap-6 overflow-x-auto px-5 sm:px-8">
      {nav.map((item) => {
        const active = pathname === item.href || pathname.startsWith(`${item.href}/`);
        return (
          <Link key={item.href} href={item.href} aria-current={active ? "page" : undefined}
            className={`shrink-0 border-b-2 py-3.5 text-sm transition-colors ${active ? "border-foreground text-foreground" : "border-transparent text-muted-foreground hover:border-border hover:text-foreground"}`}>
            {item.label}
          </Link>
        );
      })}
    </nav>
  );
}
