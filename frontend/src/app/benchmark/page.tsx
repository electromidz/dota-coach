import { Benchmark } from "@/components/benchmark/Benchmark";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Benchmark" };

export default function BenchmarkPage() {
  return (
    <AppShell
      title="Benchmark"
      eyebrow="Comparison"
    >
      <Benchmark />
    </AppShell>
  );
}
