import { MatchList } from "@/components/matches/MatchList";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Matches" };

export default function MatchesPage() {
  return (
    <AppShell title="Matches">
      <MatchList />
    </AppShell>
  );
}
