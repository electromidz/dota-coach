import { InfoTip } from "@/components/ui/InfoTip";
import { cn } from "@/lib/utils";

/**
 * The frame every chart sits in: a title, an optional (i), the visual, and at
 * most one line of caption. Longer explanations belong in `info`.
 */
export function ChartCard({
  title,
  info,
  caption,
  action,
  className,
  children,
}: {
  title: string;
  info?: React.ReactNode;
  caption?: React.ReactNode;
  action?: React.ReactNode;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <section
      aria-label={title}
      className={cn("glass flex min-w-0 flex-col gap-4 rounded-card p-4 sm:p-5", className)}
    >
      <header className="flex items-center justify-between gap-3">
        <h2 className="flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.18em] text-ink-muted">
          {title}
          {info ? <InfoTip label={`About ${title}`}>{info}</InfoTip> : null}
        </h2>
        {action}
      </header>
      {children}
      {caption ? (
        <p className="truncate text-xs text-ink-faint">{caption}</p>
      ) : null}
    </section>
  );
}
