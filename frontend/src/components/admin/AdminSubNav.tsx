"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

import { cn } from "@/lib/utils";

const TABS = [
  { href: "/admin", label: "Dashboard" },
  { href: "/admin/users", label: "Users" },
  { href: "/admin/vouchers", label: "Vouchers" },
] as const;

/**
 * The three admin destinations. Kept out of the app's global nav entirely —
 * see `lib/nav.ts` — since almost every account never sees this section at
 * all; a local tab strip is the right amount of chrome for the few screens
 * one role uses.
 */
export function AdminSubNav() {
  const pathname = usePathname();

  return (
    <nav aria-label="Admin sections" className="flex gap-2">
      {TABS.map((tab) => {
        // Exact match for "/admin" so it does not also light up on "/admin/users".
        const active =
          tab.href === "/admin" ? pathname === "/admin" : pathname.startsWith(tab.href);

        return (
          <Link
            key={tab.href}
            href={tab.href}
            aria-current={active ? "page" : undefined}
            className={cn(
              "focus-neon min-h-11 cursor-pointer rounded-xl border px-4 py-2 text-sm",
              "transition-colors duration-200 ease-out",
              active
                ? "border-function/60 bg-function/10 text-ink"
                : "border-glass-edge text-ink-muted hover:text-ink",
            )}
          >
            {tab.label}
          </Link>
        );
      })}
    </nav>
  );
}
