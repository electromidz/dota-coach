"use client";

import { TabBar } from "@/components/shell/TabBar";
import { useSession } from "@/lib/session-context";

/**
 * Shows the tab bar only once there is somewhere to navigate to.
 *
 * A native app does not show its chrome before you are signed in, and the bar
 * stays hidden while the session is still resolving so it cannot flash in and
 * straight back out.
 */
export function TabBarGate() {
  const { session } = useSession();
  return session.kind === "signed-in" ? <TabBar /> : null;
}
