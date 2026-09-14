import { Coach } from "@/components/coach/Coach";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Coach" };

export default function CoachPage() {
  return (
    <AppShell
      title="Coach"
      eyebrow="AI coaching"
      description="What your numbers say, and what to do about it."
    >
      <Coach />
    </AppShell>
  );
}
