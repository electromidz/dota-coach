"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

import { Icon, type IconName } from "@/components/ui/Icon";
import { cn } from "@/lib/utils";

const TABS: { href: string; label: string; icon: IconName }[] = [
  { href: "/admin", label: "Dashboard", icon: "gauge" },
  { href: "/admin/users", label: "Users", icon: "users" },
  { href: "/admin/vouchers", label: "Vouchers", icon: "ticket" },
];

/**
 * The admin tabs. Rendered once by the admin layout, so they stay mounted
 * while the content below changes. `prefetch` loads each tab's route ahead
 * of the click (production builds only; dev renders on demand).
 */
export function AdminSubNav() {
  const pathname = usePathname();

  return (
    <nav aria-label="Admin sections" className="-mx-1 overflow-x-auto px-1">
      <ul className="flex gap-2">
        {TABS.map((tab) => {
          // Exact for the dashboard; a prefix for the others, so a user or
          // voucher detail page still lights up its tab.
          const active =
            tab.href === "/admin" ? pathname === "/admin" : pathname.startsWith(tab.href);

          return (
            <li key={tab.href}>
              <Link
                href={tab.href}
                prefetch
                aria-current={active ? "page" : undefined}
                className={cn(
                  "focus-neon flex min-h-11 cursor-pointer items-center gap-2 whitespace-nowrap rounded-xl border px-4 text-sm",
                  "transition-colors duration-200 ease-out",
                  active
                    ? "border-function/60 bg-function/10 text-ink"
                    : "border-glass-edge text-ink-muted hover:text-ink",
                )}
              >
                <Icon name={tab.icon} className="size-4" />
                {tab.label}
              </Link>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
