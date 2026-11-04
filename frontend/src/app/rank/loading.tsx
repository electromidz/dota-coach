import { AppShell } from "@/components/shell/AppShell";
import { PageSkeleton } from "@/components/shell/PageSkeleton";

/** Shown while the route's code and data are still on the way. */
export default function Loading() {
  return (
    <AppShell>
      <PageSkeleton />
    </AppShell>
  );
}
