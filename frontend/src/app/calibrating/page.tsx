import { Calibrating } from "@/components/calibrating/Calibrating";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Calibrating" };

export default function CalibratingPage() {
  return (
    <AppShell
      title="Calibrating"
      eyebrow="Rank"
      description="Where your rank actually sits, how settled it is, and how it got there."
    >
      <Calibrating />
    </AppShell>
  );
}
