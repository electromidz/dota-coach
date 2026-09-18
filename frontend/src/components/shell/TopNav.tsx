"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

import { Brand } from "@/components/shell/Brand";
import { Icon } from "@/components/ui/Icon";
import { NAV_ITEMS, isActiveRoute } from "@/lib/nav";
import { useSession } from "@/lib/session-context";
import { cn, formatRank } from "@/lib/utils";

/**
 * The admin panel is not in `NAV_ITEMS`: almost no account ever sees it, and
 * a link every visitor sees but only one role can use is worse than no link
 * at all. Shown only to `is_admin` accounts, and only here — the phone tab
 * bar stays exactly the six destinations everyone gets.
 */
function AdminLink({ active }: { active: boolean }) {
  return (
    <Link
      href="/admin"
      aria-current={active ? "page" : undefined}
      className={cn(
        "focus-neon flex cursor-pointer items-center gap-2 rounded-xl border px-3.5 py-2",
        "text-sm font-medium tracking-wide",
        "transition-[color,background-color,border-color] duration-200 ease-out",
        active
          ? "border-keyword/60 bg-keyword/10 text-keyword"
          : "border-transparent text-ink-faint hover:border-glass-edge hover:text-ink",
      )}
    >
      <Icon name="shield" className="size-4" />
      Admin
    </Link>
  );
}

/**
 * Desktop header: brand, horizontal navigation, account chip.
 *
 * Phones never see this — they navigate from the bottom tab bar, which is
 * reachable one-handed. Above `lg` the bottom bar would be a long way from the
 * pointer and would waste a fixed strip of a tall window, so the same
 * destinations move up here instead.
 */
export function TopNav() {
  const pathname = usePathname();
  const { session } = useSession();
  const me = session.kind === "signed-in" ? session.me : null;
  const rank = me ? formatRank(me.dota_player.rank_tier) : null;

  return (
    <header className="glass sticky top-0 z-30 hidden rounded-none border-x-0 border-t-0 lg:block">
      <div className="mx-auto flex h-16 max-w-7xl items-center gap-8 px-8">
        <Brand />

        <nav aria-label="Primary" className="min-w-0 flex-1">
          <ul className="flex items-center gap-1">
            {NAV_ITEMS.map((item) => {
              const active = isActiveRoute(pathname, item.href);

              return (
                <li key={item.href}>
                  <Link
                    href={item.href}
                    aria-current={active ? "page" : undefined}
                    className={cn(
                      "focus-neon flex cursor-pointer items-center gap-2 rounded-xl px-3.5 py-2",
                      "text-sm font-medium tracking-wide",
                      "transition-[color,background-color,box-shadow] duration-200 ease-out",
                      active
                        ? "soft-pressed bg-surface-2/70 text-function"
                        : "text-ink-faint hover:bg-surface-2/40 hover:text-ink",
                    )}
                  >
                    <Icon name={item.icon} className="size-4" />
                    {item.label}
                  </Link>
                </li>
              );
            })}
          </ul>
        </nav>

        {me?.user.is_admin ? <AdminLink active={pathname.startsWith("/admin")} /> : null}

        {me ? (
          <Link
            href="/profile"
            className={cn(
              "focus-neon flex shrink-0 cursor-pointer items-center gap-3 rounded-xl px-2 py-1.5",
              "transition-colors duration-200 ease-out hover:bg-surface-2/40",
            )}
          >
            <span className="flex min-w-0 flex-col items-end leading-tight">
              <span className="max-w-40 truncate text-sm text-ink">
                {me.user.persona_name ?? "Steam player"}
              </span>
              <span className="text-[0.6875rem] text-ink-faint">
                {rank ?? "Rank unknown"}
              </span>
            </span>

            {me.user.avatar_url ? (
              // Plain <img>: the avatar host is not in the Next image
              // allowlist, and this is one small image.
              // eslint-disable-next-line @next/next/no-img-element
              <img
                src={me.user.avatar_url}
                alt=""
                className="size-9 shrink-0 rounded-full border border-glass-edge object-cover"
              />
            ) : (
              <span className="flex size-9 shrink-0 items-center justify-center rounded-full border border-glass-edge bg-surface-2 font-display text-sm text-ink-faint">
                {(me.user.persona_name ?? "?").slice(0, 1).toUpperCase()}
              </span>
            )}
          </Link>
        ) : null}
      </div>
    </header>
  );
}
