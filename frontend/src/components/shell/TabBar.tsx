"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

import { Icon, type IconName } from "@/components/ui/Icon";
import { cn } from "@/lib/utils";

const TABS: Array<{ href: string; label: string; icon: IconName }> = [
  { href: "/", label: "Overview", icon: "home" },
  { href: "/matches", label: "Matches", icon: "swords" },
  { href: "/profile", label: "Profile", icon: "user" },
];

/**
 * Fixed bottom tab bar — the primary navigation on both iOS and Android.
 *
 * Sits above the home indicator via the safe-area inset, blurs the content
 * behind it, and marks the active tab with an icon fill *and* a label colour,
 * so the current tab is never signalled by colour alone.
 */
export function TabBar() {
  const pathname = usePathname();

  return (
    <nav
      aria-label="Primary"
      className="glass safe-bottom fixed inset-x-0 bottom-0 z-30 rounded-none border-x-0 border-b-0"
    >
      <ul className="mx-auto flex max-w-lg items-stretch">
        {TABS.map((tab) => {
          // "/" must match exactly or it would light up on every route.
          const active =
            tab.href === "/" ? pathname === "/" : pathname.startsWith(tab.href);

          return (
            <li key={tab.href} className="flex-1">
              <Link
                href={tab.href}
                aria-current={active ? "page" : undefined}
                className={cn(
                  "focus-neon flex h-16 cursor-pointer flex-col items-center justify-center gap-1",
                  "transition-colors duration-200 ease-out",
                  active ? "text-function" : "text-ink-faint hover:text-ink",
                )}
              >
                <span className="relative">
                  {active ? (
                    <span
                      aria-hidden
                      className="absolute -inset-2 rounded-full bg-function/15 blur-[6px]"
                    />
                  ) : null}
                  <Icon name={tab.icon} className="relative size-6" />
                </span>
                <span className="text-[0.6875rem] font-medium tracking-wide">
                  {tab.label}
                </span>
              </Link>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
