"use client";

import { Component } from "react";

import { Icon } from "@/components/ui/Icon";

/**
 * One section of a page that could not load. Names the section and offers a
 * retry, so the rest of the page keeps working around it.
 */
export function SectionError({
  section,
  message,
  requestId,
  onRetry,
}: {
  section: string;
  message?: string | null;
  /** The server's id for the failed request, so a report can quote it. */
  requestId?: string | null;
  onRetry?: () => void;
}) {
  return (
    <div
      role="alert"
      className="glass neon-ring-error flex flex-wrap items-center gap-3 rounded-card p-4"
    >
      <Icon name="alert" className="size-4 shrink-0 text-error" />
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="text-sm text-ink">{section} couldn&apos;t load</span>
        {message ? (
          <span className="truncate text-xs text-ink-faint">{message}</span>
        ) : null}
        {requestId ? (
          <span className="select-all font-mono text-[0.625rem] text-ink-faint">
            Request {requestId}
          </span>
        ) : null}
      </div>
      {onRetry ? (
        <button
          type="button"
          onClick={onRetry}
          className="focus-neon flex min-h-9 cursor-pointer items-center gap-1.5 rounded-lg border border-glass-edge px-3 text-xs text-ink transition-colors duration-200 hover:bg-ink/10"
        >
          <Icon name="refresh" className="size-3.5" />
          Retry
        </button>
      ) : null}
    </div>
  );
}

/**
 * Catches a render failure inside one section so it cannot blank the page.
 * Retry remounts the section, which re-runs its own data fetch.
 */
export class SectionBoundary extends Component<
  { section: string; children: React.ReactNode },
  { failed: boolean; attempt: number }
> {
  state = { failed: false, attempt: 0 };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch(error: unknown) {
    console.error(`[${this.props.section}]`, error);
  }

  render() {
    if (this.state.failed) {
      return (
        <SectionError
          section={this.props.section}
          onRetry={() =>
            this.setState((s) => ({ failed: false, attempt: s.attempt + 1 }))
          }
        />
      );
    }
    return <div key={this.state.attempt} className="contents">{this.props.children}</div>;
  }
}
