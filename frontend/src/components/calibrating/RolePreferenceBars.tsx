import { BarList } from "@/components/charts/BarList";
import type { RolePreference } from "@/lib/types";

/**
 * Which positions the player actually queues, by share of ranked matches.
 *
 * `BarList` rather than a new chart: this is magnitude across a handful of
 * named things, which is exactly what it exists for. The percentage is the
 * annotation and the match count is the bar, so a 100% that rests on two games
 * cannot read as a settled preference.
 */
export function RolePreferenceBars({
  roles,
  className,
}: {
  roles: RolePreference[];
  className?: string;
}) {
  if (roles.length === 0) {
    return (
      <p className="text-sm text-ink-faint">
        No recent ranked matches to read a role split from.
      </p>
    );
  }

  return (
    <BarList
      className={className}
      caption="Share of recent ranked matches by role"
      data={roles.map((role) => ({
        label: role.role,
        value: role.matches,
        meta: `${role.pct.toFixed(0)}%`,
      }))}
    />
  );
}
