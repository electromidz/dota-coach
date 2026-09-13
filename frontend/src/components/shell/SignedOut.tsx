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

/**
 * The signed-out screen. No tab bar — there is nothing to navigate to yet, and
 * a native app does not show its chrome before you are in.
 */
export function SignedOut({ loginError }: { loginError?: string }) {
  const message = loginError ? LOGIN_ERRORS[loginError] : null;

  return (
    <div className="flex flex-col gap-8 py-6">
      <div className="flex flex-col gap-4">
        <p className="neon-text-function text-xs font-semibold uppercase tracking-[0.25em]">
          AI Dota Coach
        </p>

        <h1 className="font-display text-4xl leading-tight tracking-wide">
          Your personal
          <br />
          <span className="bg-gradient-to-r from-keyword via-operator to-function bg-clip-text text-transparent">
            Dota 2 coach
          </span>
        </h1>

        {/* The one place text streams: it previews how the coach will speak. */}
        <p className="text-lg leading-relaxed text-ink-muted">
          <StreamingText text="AI that watches your games, learns your habits, and tells you what to improve next." />
        </p>
      </div>

      {/* An unknown code still surfaces something, rather than failing silently. */}
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
  );
}
