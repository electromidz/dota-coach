"use client";

import { useSession } from "@/lib/session-context";

/**
 * The page header: eyebrow, title, one-line description, and the page's
 * primary action on the right.
 *
 * On a phone this collapses to nothing visible — the title is already in the
 * sticky strip and the action is in the body, where a thumb can reach it — but
 * the `<h1>` stays in the accessibility tree so every screen announces one, at
 * one level, at every width.
 *
 * Gated on the session for the same reason the nav is: a signed-out visitor
 * gets the marketing screen, which brings its own `<h1>`, and two would be one
 * too many.
 */
export function PageHeader({
  title,
  eyebrow,
  description,
  action,
}: {
  title: React.ReactNode;
  eyebrow?: string;
  description?: string;
  action?: React.ReactNode;
}) {
  const { session } = useSession();
  if (session.kind !== "signed-in") return null;

  return (
    <div className="sr-only lg:not-sr-only lg:mb-8 lg:flex lg:flex-wrap lg:items-end lg:justify-between lg:gap-6">
      <div className="min-w-0">
        {eyebrow ? (
          <p
            aria-hidden
            className="hidden items-center gap-2 text-xs font-semibold uppercase tracking-[0.25em] text-function lg:flex"
          >
            <span className="size-1.5 rounded-full bg-function shadow-[0_0_8px_var(--color-function)]" />
            {eyebrow}
          </p>
        ) : null}

        <h1 className="font-display text-3xl tracking-wide lg:mt-2.5">
          {title}
        </h1>

        {description ? (
          <p className="hidden text-sm text-ink-muted lg:mt-2 lg:block">
            {description}
          </p>
        ) : null}
      </div>

      {action ? <div className="hidden shrink-0 lg:block">{action}</div> : null}
    </div>
  );
}
