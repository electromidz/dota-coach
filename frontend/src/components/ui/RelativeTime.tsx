import { fullDate, timeAgo } from "@/lib/utils";

/**
 * "9d ago", "5w ago" — one relative format everywhere, with the full date
 * on hover and in the machine-readable `dateTime`.
 */
export function RelativeTime({ iso, className }: { iso: string; className?: string }) {
  return (
    <time dateTime={iso} title={fullDate(iso)} className={className}>
      {timeAgo(iso)}
    </time>
  );
}
