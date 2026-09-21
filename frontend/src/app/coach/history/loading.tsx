import { AppShell } from "@/components/shell/AppShell";
import { PageSkeleton } from "@/components/shell/PageSkeleton";

export default function Loading() {
  return (
    <AppShell title="Coaching history" eyebrow="Progress">
      <PageSkeleton rows={4} />
    </AppShell>
  );
}
