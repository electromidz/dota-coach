import { HeroIntelligence } from "@/components/heroes/HeroIntelligence";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Heroes" };

export default function HeroesPage() {
  return (
    <AppShell
      title="Heroes"
      eyebrow="Hero intelligence"
    >
      <HeroIntelligence />
    </AppShell>
  );
}
