import { CoachingHistory } from "@/components/coach/CoachingHistory";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Coaching history" };

export default function CoachingHistoryPage() {
  return (
    <AppShell
      title="Coaching history"
      eyebrow="Progress"
      description="Every session, exactly as it was measured at the time."
    >
      <CoachingHistory />
    </AppShell>
  );
}
