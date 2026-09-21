import { SessionDetail } from "@/components/coach/SessionDetail";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Coaching session" };

/**
 * A pushed detail screen: no tab bar, a back affordance instead — the same
 * convention `/matches/[id]` uses.
 */
export default async function SessionPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;

  return (
    <AppShell showTabs={false}>
      <SessionDetail id={id} />
    </AppShell>
  );
}
