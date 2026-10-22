import Link from "next/link";

import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";

/**
 * Shown in place of a feature's content for a trial-expired, unsubscribed
 * account. A state, not an error: nothing failed, the account simply has no
 * active entitlement, and the same screen worked yesterday and will again
 * once it subscribes.
 */
export function Paywall({
  title = "Subscribe to unlock this",
  message = "This feature needs an active trial or subscription.",
}: {
  title?: string;
  message?: string;
}) {
  return (
    <Card className="flex flex-col items-start gap-3" glow="keyword">
      <Icon name="shield" className="size-6 text-function" />
      <h2 className="font-display text-sm uppercase tracking-[0.2em] text-ink-faint">
        {title}
      </h2>
      <p className="text-sm leading-relaxed text-ink-muted">{message}</p>
      <Link
        href="/billing"
        className="focus-neon w-fit cursor-pointer rounded text-sm text-function transition-colors duration-200 ease-out hover:text-ink"
      >
        See the plan →
      </Link>
    </Card>
  );
}
