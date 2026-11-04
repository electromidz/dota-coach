import { Calibrating } from "@/components/calibrating/Calibrating";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Rank" };

export default function RankPage() {
  return (
    <AppShell
      title="Rank"
      description="Where your rank actually sits, how settled it is, and how it got there."
    >
      <Calibrating />
    </AppShell>
  );
}
