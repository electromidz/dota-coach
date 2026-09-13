import { Brand } from "@/components/shell/Brand";
import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { SteamLoginButton } from "@/components/ui/SteamLoginButton";
import { StreamingText } from "@/components/ui/StreamingText";
import { SystemStatus } from "@/components/ui/SystemStatus";

/** Codes the login redirect can hand back on `?error=`. */
const LOGIN_ERRORS: Record<string, string> = {
  login_expired: "That sign-in attempt expired. Please try again.",
  steam_rejected: "Steam could not verify that sign-in. Please try again.",
  steam_unavailable: "Steam is not responding right now. Try again shortly.",
  no_dota_account: "That Steam account has no Dota 2 player ID.",
  server_error: "Something went wrong signing you in. Please try again.",
};

/** What the product does, in the order a first-time visitor asks it. */
const PROMISES = [
  {
    title: "Every match, read for you",
    body: "Laning, economy and deaths turned into numbers you can act on.",
  },
  {
    title: "Measured against your peers",
    body: "Benchmarks for your rank and role, never a global average.",
  },
  {
    title: "One thing to train next",
    body: "The coach picks a single focus and tracks whether it moves.",
  },
];

/**
 * The signed-out screen — the only page an unauthenticated visitor sees, so it
 * is both the login and the pitch.
 *
 * There is no tab bar and no header nav here: there is nothing to navigate to
 * yet, and a native app does not show its chrome before you are in. On a phone
 * this is one column ending in the Steam button; from `lg` the pitch and the
 * sign-in panel sit side by side so neither is below the fold.
 */
export function SignedOut({ loginError }: { loginError?: string }) {
  const message = loginError ? LOGIN_ERRORS[loginError] : null;

  return (
    <div className="flex flex-col gap-8 py-6 lg:grid lg:grid-cols-2 lg:items-center lg:gap-16 lg:py-16">
      <div className="flex flex-col gap-4">
        <Brand className="mb-2 lg:mb-4" />

        <p className="neon-text-function text-xs font-semibold uppercase tracking-[0.25em]">
          AI Dota Coach
        </p>

        <h1 className="font-display text-4xl leading-tight tracking-wide lg:text-6xl">
          Your personal
          <br />
          <span className="bg-gradient-to-r from-keyword via-operator to-function bg-clip-text text-transparent">
            Dota 2 coach
          </span>
        </h1>

        {/* The one place text streams: it previews how the coach will speak. */}
        <p className="text-lg leading-relaxed text-ink-muted lg:max-w-md lg:text-xl">
          <StreamingText text="AI that watches your games, learns your habits, and tells you what to improve next." />
        </p>

        <ul className="mt-2 hidden flex-col gap-4 lg:flex">
          {PROMISES.map((promise) => (
            <li key={promise.title} className="flex gap-3">
              <span
                aria-hidden
                className="mt-2 size-1.5 shrink-0 rounded-full bg-function shadow-[0_0_8px_var(--color-function)]"
              />
              <span className="flex flex-col gap-0.5">
                <span className="text-sm font-semibold text-ink">
                  {promise.title}
                </span>
                <span className="text-sm text-ink-muted">{promise.body}</span>
              </span>
            </li>
          ))}
        </ul>
      </div>

      <div className="flex flex-col gap-6 lg:glass lg:rounded-card lg:p-8">
        {/* An unknown code still surfaces something, rather than failing
            silently. */}
        {loginError ? (
          <Alert title="Sign-in did not complete">
            {message ?? "Please try signing in again."}
          </Alert>
        ) : null}

        <SteamLoginButton className="w-full" />

        <p className="text-xs leading-relaxed text-ink-faint">
          We only read your public Dota 2 match history. Signing in with Steam
          never shares your password with this app.
        </p>

        <section className="flex flex-col gap-3">
          <h2 className="text-xs font-semibold uppercase tracking-[0.2em] text-ink-faint">
            Service status
          </h2>
          <Card>
            <SystemStatus />
          </Card>
        </section>
      </div>
    </div>
  );
}
