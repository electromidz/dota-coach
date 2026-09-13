"use client";

import { ProfileCard } from "@/components/dashboard/ProfileCard";
import { SignedOut } from "@/components/shell/SignedOut";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import { logout } from "@/lib/api";
import { useSession } from "@/lib/session-context";

export function Profile() {
  const { session } = useSession();

  async function handleLogout() {
    await logout().catch(() => undefined);
    window.location.href = "/";
  }

  if (session.kind === "loading") {
    return (
      <div className="flex flex-col gap-4" aria-busy="true">
        <span className="sr-only">Loading profile…</span>
        <div className="h-52 animate-pulse rounded-card bg-surface-2" />
      </div>
    );
  }

  if (session.kind === "anonymous") return <SignedOut />;
  if (session.kind === "error") {
    return <Alert title="Cannot reach the service">{session.message}</Alert>;
  }

  const { me } = session;

  return (
    <div className="flex flex-col gap-5 pb-4">
      <ProfileCard user={me.user} dotaPlayer={me.dota_player} />

      <Card className="flex items-center justify-between gap-3">
        <div>
          <p className="text-sm text-ink">Matches stored</p>
          <p className="text-xs text-ink-faint">In your local history</p>
        </div>
        <span className="font-mono text-xl tabular-nums text-number">
          {me.matches_stored}
        </span>
      </Card>

      <Button variant="ghost" onClick={handleLogout} className="w-full">
        <Icon name="logout" className="size-5" />
        Sign out
      </Button>

      <p className="text-xs leading-relaxed text-ink-faint">
        Roles and scores shown in this app are estimates derived from public
        match data. They are not an official rating and do not promise MMR
        gains.
      </p>
    </div>
  );
}
