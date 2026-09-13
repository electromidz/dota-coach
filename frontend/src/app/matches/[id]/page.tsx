import { MatchDetail } from "@/components/matches/MatchDetail";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Match" };

/**
 * A pushed detail screen: no tab bar, a back affordance instead — the same
 * convention as a native navigation stack.
 */
export default async function MatchPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;

  return (
    <AppShell showTabs={false}>
      <MatchDetail id={id} />
    </AppShell>
  );
}
