import Link from "next/link";

import { AppShell } from "@/components/shell/AppShell";
import { Alert } from "@/components/ui/Alert";

export const metadata = { title: "Not found" };

/** A URL that does not exist. The tab bar stays, so it is one tap back. */
export default function NotFound() {
  return (
    <AppShell title="Not found" eyebrow="404">
      <div className="flex flex-col gap-5 pb-4">
        <Alert tone="info" title="There is nothing at this address">
          The link may be old, or the match may not be in your synced history.
        </Alert>

        <Link
          href="/"
          className="focus-neon w-fit text-sm text-function underline-offset-4 hover:underline"
        >
          Back to the overview
        </Link>
      </div>
    </AppShell>
  );
}
