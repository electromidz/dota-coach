"use client";

import { SignedOut } from "@/components/shell/SignedOut";
import { PageSkeleton } from "@/components/shell/PageSkeleton";
import { Alert } from "@/components/ui/Alert";
import { useSession } from "@/lib/session-context";

/**
 * The one gate every admin screen goes through.
 *
 * `is_admin` is set only by hand in the database — there is no self-serve
 * promotion path — so a non-admin signed-in account sees a plain refusal
 * rather than a broken table: the backend would 403 every call this screen
 * makes, and rendering the shell around that would just be a slower way to
 * say the same thing.
 */
export function AdminGate({ children }: { children: React.ReactNode }) {
  const { session } = useSession();

  if (session.kind === "loading") return <PageSkeleton />;
  if (session.kind === "anonymous") return <SignedOut />;
  if (session.kind === "error") {
    return <Alert title="Cannot reach the service">{session.message}</Alert>;
  }

  if (!session.me.user.is_admin) {
    return (
      <Alert title="Admin access required">
        This account does not have access to the admin panel.
      </Alert>
    );
  }

  return <>{children}</>;
}
