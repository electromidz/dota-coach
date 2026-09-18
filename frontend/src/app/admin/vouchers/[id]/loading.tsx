import { AppShell } from "@/components/shell/AppShell";
import { PageSkeleton } from "@/components/shell/PageSkeleton";

export default function Loading() {
  return (
    <AppShell showTabs={false}>
      <PageSkeleton />
    </AppShell>
  );
}
