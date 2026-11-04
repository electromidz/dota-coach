import { scopeText, type Scope } from "@/lib/utils";

/**
 * What a section's numbers are computed over, in one line:
 * `hero · role · mode · N matches`. Shown under every section heading so two
 * pages that disagree can be seen to be answering different questions.
 */
export function ScopeLine({ className, ...scope }: Scope & { className?: string }) {
  return (
    <p
      className={`font-mono text-[0.6875rem] tabular-nums text-ink-faint ${className ?? ""}`}
      data-testid="scope-line"
    >
      {scopeText(scope)}
    </p>
  );
}
