"use client";

import { PageHeader } from "@/components/shell/PageHeader";
import { useSession } from "@/lib/session-context";
import { cn } from "@/lib/utils";

/**
 * The page's `<main>`, gated on session the same way `AppChrome` and
 * `AppBackdrop` are.
 *
 * An anonymous visitor sees `SignedOut` — a full-bleed marketing page with its
 * own nav, sections and footer — so it cannot sit inside the dashboard's
 * padded, width-capped column, and it must never appear under a "Dashboard /
 * Overview" heading it did not ask for. `loading` and `error` stay in the
 * normal wrapper: they render dashboard-shaped skeletons and alerts that are
 * meant to fit the space a real page would occupy.
 */
export function AppMain({
  title,
  eyebrow,
  description,
  action,
  showTabs,
  children,
}: {
  title?: React.ReactNode;
  eyebrow?: string;
  description?: string;
  action?: React.ReactNode;
  showTabs: boolean;
  children: React.ReactNode;
}) {
  const { session } = useSession();

  if (session.kind === "anonymous") return <main>{children}</main>;

  return (
    <main
      className={cn(
        "safe-x mx-auto w-full max-w-lg pt-5 lg:max-w-7xl lg:pt-8",
        showTabs ? "app-scroll" : "pb-10",
      )}
    >
      {title ? (
        <PageHeader
          title={title}
          eyebrow={eyebrow}
          description={description}
          action={action}
        />
      ) : null}

      {children}
    </main>
  );
}
