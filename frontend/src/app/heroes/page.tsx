import { HeroIntelligence } from "@/components/heroes/HeroIntelligence";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Heroes" };

export default function HeroesPage() {
  return (
    <AppShell
      title="Heroes"
      eyebrow="Hero intelligence"
      description="Which of the currently strong heroes actually fit you."
    >
      <HeroIntelligence />
    </AppShell>
  );
}
