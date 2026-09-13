"use client";

import { TabBar } from "@/components/shell/TabBar";
import { TopNav } from "@/components/shell/TopNav";
import { useSession } from "@/lib/session-context";

/**
 * The navigation chrome, shown only once there is somewhere to navigate to.
 *
 * A native app does not show its chrome before you are signed in, and it stays
 * hidden while the session is still resolving so it cannot flash in and
 * straight back out. One gate covers every piece — the desktop header, the
 * phone title strip, the tab bar — so they can never disagree about whether
 * the visitor is signed in.
 *
 * The title strip is `aria-hidden`: it is a visual echo of the `<h1>` that
 * `PageHeader` puts in the document, not a second heading.
 */
export function AppChrome({
  title,
  action,
  showTabs = true,
}: {
  title?: React.ReactNode;
  action?: React.ReactNode;
  showTabs?: boolean;
}) {
  const { session } = useSession();
  if (session.kind !== "signed-in") return null;

  return (
    <>
      <TopNav />

      {title ? (
        <header className="glass safe-top sticky top-0 z-20 rounded-none border-x-0 border-t-0 lg:hidden">
          <div className="safe-x mx-auto flex h-14 max-w-lg items-center justify-between gap-3">
            <span
              aria-hidden
              className="truncate font-display text-lg tracking-wide"
            >
              {title}
            </span>
            {action}
          </div>
        </header>
      ) : null}

      {showTabs ? <TabBar /> : null}
    </>
  );
}
