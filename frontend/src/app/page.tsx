import type { Metadata } from "next";
import { cookies } from "next/headers";

import { Overview } from "@/components/dashboard/Overview";
import { AppShell } from "@/components/shell/AppShell";
import { SignedOut } from "@/components/shell/SignedOut";
import { fetchPlan } from "@/lib/plan-server";
import { SITE_DESCRIPTION, SITE_TITLE } from "@/lib/site";

/**
 * Written for the landing page, because that is what this URL serves to
 * anyone without a session — which includes every crawler. The signed-in
 * dashboard shares the URL but is behind a login and carries no search
 * intent, so the title and description describe the public page.
 */
export const metadata: Metadata = {
  title: {
    absolute: SITE_TITLE,
  },
  description: SITE_DESCRIPTION,
  alternates: { canonical: "/" },
  openGraph: {
    title: SITE_TITLE,
    description: SITE_DESCRIPTION,
    url: "/",
  },
};

/**
 * `/` is two pages wearing one URL: the marketing landing page for visitors,
 * the dashboard for players.
 *
 * It used to pick between them in the browser, after `SessionProvider` had
 * resolved `/api/auth/me`. That made the served HTML a loading skeleton for
 * everyone, crawlers included — the entire pitch, every heading and the FAQ
 * existed only after hydration. Deciding here instead means an unauthenticated
 * request gets the finished landing page in its first byte.
 *
 * The decision uses `dc_signed_in`, a hint cookie the client sets on this
 * origin (the real session cookie is `HttpOnly` and belongs to the backend
 * host, so this server cannot read it). The hint is not trusted for anything:
 * the dashboard branch still resolves the real session client-side, and
 * `Overview` still falls back to `SignedOut` if that comes back anonymous. A
 * forged hint shows a stranger a skeleton, never data.
 */
export default async function HomePage({
  searchParams,
}: {
  searchParams: Promise<{ error?: string }>;
}) {
  const [{ error }, jar] = await Promise.all([searchParams, cookies()]);

  if (!jar.get("dc_signed_in")?.value) {
    // Price and trial length ship inside the HTML rather than appearing a
    // round trip later; `null` when the backend is unreachable, which every
    // consumer already renders as "no figure" rather than a guess.
    const plan = await fetchPlan();

    return <SignedOut loginError={error} plan={plan} />;
  }

  return (
    <AppShell
      title="Overview"
      eyebrow="Dashboard"
      description="Your latest matches and what they say about your play."
    >
      <Overview loginError={error} />
    </AppShell>
  );
}
