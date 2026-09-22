import { DashboardMockup } from "@/components/landing/DashboardMockup";
import { TrialOffer } from "@/components/shell/TrialOffer";
import { Alert } from "@/components/ui/Alert";
import { ButtonLink } from "@/components/ui/Button";
import { Icon } from "@/components/ui/Icon";
import { steamLoginUrl } from "@/lib/api";
import type { PlanResponse } from "@/lib/types";

/** Codes the login redirect can hand back on `?error=`. */
const LOGIN_ERRORS: Record<string, string> = {
  login_expired: "That sign-in attempt expired. Please try again.",
  steam_rejected: "Steam could not verify that sign-in. Please try again.",
  steam_unavailable: "Steam is not responding right now. Try again shortly.",
  no_dota_account: "That Steam account has no Dota 2 player ID.",
  server_error: "Something went wrong signing you in. Please try again.",
};

/**
 * The one `<h1>` on the site, so it is the strongest on-page signal there is
 * for what this page is about. It names the thing being searched for — "AI
 * Dota 2 coach" — before the hook rather than after it: "Stop losing to the
 * same mistake" was the better line and the worse heading, because nothing in
 * it said Dota. The hook survives as the second half of the sentence.
 */
export function Hero({
  loginError,
  plan,
}: {
  loginError?: string;
  plan?: PlanResponse | null;
}) {
  const message = loginError ? LOGIN_ERRORS[loginError] : null;

  return (
    <section className="safe-x relative mx-auto grid max-w-7xl gap-12 pt-10 pb-16 lg:grid-cols-2 lg:items-center lg:gap-10 lg:pt-20 lg:pb-28">
      <div className="flex flex-col gap-6">
        {loginError ? (
          <Alert title="Sign-in did not complete">
            {message ?? "Please try signing in again."}
          </Alert>
        ) : null}

        <span className="flex w-fit items-center gap-2 rounded-full border border-glass-edge bg-surface-2/60 px-3 py-1.5 text-xs font-medium tracking-wide text-ink-muted">
          <span className="relative flex size-1.5">
            <span className="absolute inline-flex size-full animate-ping rounded-full bg-string opacity-75" />
            <span className="relative inline-flex size-1.5 rounded-full bg-string" />
          </span>
          Dota 2 coaching that updates with the current patch
        </span>

        <h1 className="font-display text-4xl leading-[1.08] tracking-wide sm:text-5xl lg:text-6xl">
          The AI Dota 2 coach that finds
          <br />
          <span className="bg-gradient-to-r from-keyword via-operator to-function bg-clip-text text-transparent">
            the mistake you keep making
          </span>
        </h1>

        <p className="max-w-lg text-lg leading-relaxed text-ink-muted lg:text-xl">
          Sign in with Steam and Dota Coach reads your recent{" "}
          <strong className="font-medium text-ink">
            Dota 2 match history
          </strong>
          , benchmarks it against players at your own rank and role, and turns
          what keeps costing you games into{" "}
          <strong className="font-medium text-ink">
            one thing to fix next
          </strong>{" "}
          — with the numbers to prove it moved.
        </p>

        <div className="flex flex-col gap-4 sm:flex-row sm:items-center">
          <ButtonLink href={steamLoginUrl()} className="w-full sm:w-auto">
            <Icon name="steam" className="size-5" />
            Start free trial — no card
          </ButtonLink>
          <ButtonLink
            href="#product"
            variant="ghost"
            className="w-full sm:w-auto"
          >
            See a sample coaching report
            <Icon name="external" className="size-4" />
          </ButtonLink>
        </div>

        <TrialOffer
          initialPlan={plan}
          className="text-sm leading-relaxed text-ink-faint"
        />

        <p className="text-xs text-ink-faint">
          Cancel anytime · Works with your Steam account · We only read public
          Dota 2 match history
        </p>
      </div>

      <DashboardMockup />
    </section>
  );
}
