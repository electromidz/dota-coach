import { Benchmark } from "@/components/benchmark/Benchmark";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Benchmark" };

export default function BenchmarkPage() {
  return (
    <AppShell
      title="Benchmark"
      eyebrow="Comparison"
      description="How your numbers sit against other players on the same hero."
    >
      <Benchmark />
    </AppShell>
  );
}
