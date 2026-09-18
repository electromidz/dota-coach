import { DashboardMockup } from "@/components/landing/DashboardMockup";
import { TrialOffer } from "@/components/shell/TrialOffer";
import { Alert } from "@/components/ui/Alert";
import { ButtonLink } from "@/components/ui/Button";
import { Icon } from "@/components/ui/Icon";
import { steamLoginUrl } from "@/lib/api";

/** Codes the login redirect can hand back on `?error=`. */
const LOGIN_ERRORS: Record<string, string> = {
  login_expired: "That sign-in attempt expired. Please try again.",
  steam_rejected: "Steam could not verify that sign-in. Please try again.",
  steam_unavailable: "Steam is not responding right now. Try again shortly.",
  no_dota_account: "That Steam account has no Dota 2 player ID.",
  server_error: "Something went wrong signing you in. Please try again.",
};

export function Hero({ loginError }: { loginError?: string }) {
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
          Coaching that updates with the current patch
        </span>

        <h1 className="font-display text-5xl leading-[1.05] tracking-wide sm:text-6xl lg:text-7xl">
          Stop losing to the
          <br />
          <span className="bg-gradient-to-r from-keyword via-operator to-function bg-clip-text text-transparent">
            same mistake
          </span>
        </h1>

        <p className="max-w-lg text-lg leading-relaxed text-ink-muted lg:text-xl">
          Sign in with Steam and we read your last matches, find what keeps
          costing you games, and turn it into one thing to fix next — with the
          numbers to prove it moved.
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
            See a sample report
            <Icon name="external" className="size-4" />
          </ButtonLink>
        </div>

        <TrialOffer className="text-sm leading-relaxed text-ink-faint" />

        <p className="text-xs text-ink-faint">
          Cancel anytime · Works with your Steam account · We only read public
          match history
        </p>
      </div>

      <DashboardMockup />
    </section>
  );
}
