import { Overview } from "@/components/dashboard/Overview";
import { AppShell } from "@/components/shell/AppShell";

/**
 * Overview tab. The login redirect lands here with `?error=<code>` on failure.
 */
export default async function HomePage({
  searchParams,
}: {
  searchParams: Promise<{ error?: string }>;
}) {
  const { error } = await searchParams;

  return (
    <AppShell title="Overview">
      <Overview loginError={error} />
    </AppShell>
  );
}
