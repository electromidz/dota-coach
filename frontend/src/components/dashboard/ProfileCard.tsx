import { Card } from "@/components/ui/Card";
import { CopyButton } from "@/components/ui/CopyButton";
import { Icon } from "@/components/ui/Icon";
import { dotabuffUrl, openDotaUrl } from "@/lib/profile";
import type { DotaPlayer, User } from "@/lib/types";
import { cn, formatRank, timeAgo } from "@/lib/utils";

/**
 * Steam identity on the left, Dota identity below — the two things Phase 3 has
 * to prove are connected.
 */
export function ProfileCard({
  user,
  dotaPlayer,
  onSync,
  syncing = false,
}: {
  user: User;
  dotaPlayer: DotaPlayer;
  /** Omitted, the card shows no sync control. */
  onSync?: () => void;
  syncing?: boolean;
}) {
  const rank = formatRank(dotaPlayer.rank_tier);

  return (
    <Card className="flex flex-col gap-5">
      <div className="flex items-center gap-4">
        <div className="relative shrink-0">
          {/* Glow sits behind the avatar rather than on it, so the image
              itself stays unaltered. */}
          <span
            aria-hidden
            className="absolute -inset-1 rounded-full bg-keyword/25 blur-md"
          />
          {user.avatar_url ? (
            // Plain <img>: the avatar host is not in the Next image allowlist,
            // and this is one small image.
            // eslint-disable-next-line @next/next/no-img-element
            <img
              src={user.avatar_url}
              alt=""
              className="relative size-14 rounded-full border border-glass-edge object-cover"
            />
          ) : (
            <div className="relative flex size-14 items-center justify-center rounded-full border border-glass-edge bg-surface-2 font-display text-lg text-ink-faint">
              {(user.persona_name ?? "?").slice(0, 1).toUpperCase()}
            </div>
          )}
        </div>

        <div className="min-w-0 flex-1">
          <h2 className="truncate font-display text-lg tracking-wide">
            {user.persona_name ?? "Steam player"}
          </h2>
          <p className="flex flex-wrap items-center gap-x-2 text-sm text-ink-muted">
            {rank ? (
              <span className="inline-flex items-center gap-1.5 text-number">
                <Icon name="trophy" className="size-4" />
                {rank}
              </span>
            ) : (
              <span>Rank unknown</span>
            )}
            <span aria-hidden className="text-ink-faint">
              ·
            </span>
            <span>
              {dotaPlayer.last_synced_at
                ? `synced ${timeAgo(dotaPlayer.last_synced_at)}`
                : "never synced"}
            </span>
            {onSync ? (
              <button
                type="button"
                onClick={onSync}
                disabled={syncing}
                aria-busy={syncing}
                className="focus-neon inline-flex min-h-8 cursor-pointer items-center gap-1 rounded-lg px-2 text-xs font-semibold text-function transition-colors duration-200 hover:bg-ink/10 hover:text-ink disabled:cursor-not-allowed disabled:opacity-60"
              >
                <Icon
                  name="refresh"
                  className={cn("size-3.5", syncing && "animate-spin")}
                />
                {syncing ? "Syncing…" : "Sync now"}
              </button>
            ) : null}
          </p>
        </div>
      </div>

      <dl className="grid grid-cols-1 gap-3 border-t border-glass-edge pt-4 text-sm sm:grid-cols-2">
        <Field label="Dota account ID" value={String(dotaPlayer.dota_account_id)} />
        <Field label="SteamID64" value={user.steam_id} />
      </dl>

      <ul className="flex flex-wrap gap-x-5 gap-y-2" aria-label="External profiles">
        {user.profile_url ? (
          <li>
            <ExternalLink href={user.profile_url}>View Steam profile</ExternalLink>
          </li>
        ) : null}
        <li>
          <ExternalLink href={openDotaUrl(dotaPlayer.dota_account_id)}>OpenDota</ExternalLink>
        </li>
        <li>
          <ExternalLink href={dotabuffUrl(dotaPlayer.dota_account_id)}>Dotabuff</ExternalLink>
        </li>
      </ul>
    </Card>
  );
}

function ExternalLink({ href, children }: { href: string; children: React.ReactNode }) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noreferrer noopener"
      className="focus-neon inline-flex w-fit cursor-pointer items-center gap-1.5 rounded text-sm font-medium text-function transition-colors duration-200 ease-out hover:text-ink"
    >
      {children}
      <Icon name="external" className="size-4" />
      <span className="sr-only">(opens in a new tab)</span>
    </a>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3 sm:flex-col sm:items-start sm:justify-start sm:gap-1">
      <dt className="text-ink-faint">{label}</dt>
      {/* Identifiers are monospace: they are read digit by digit. */}
      <dd className="flex min-w-0 items-center gap-1">
        <span className="truncate font-mono tabular-nums text-number">{value}</span>
        <CopyButton value={value} label={label} />
      </dd>
    </div>
  );
}
