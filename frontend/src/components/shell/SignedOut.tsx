import { FAQ } from "@/components/landing/FAQ";
import { FeaturesBento } from "@/components/landing/FeaturesBento";
import { FinalCta } from "@/components/landing/FinalCta";
import { Footer } from "@/components/landing/Footer";
import { Hero } from "@/components/landing/Hero";
import { HowItWorks } from "@/components/landing/HowItWorks";
import { LandingJsonLd } from "@/components/landing/JsonLd";
import { LandingNav } from "@/components/landing/LandingNav";
import { Pricing } from "@/components/landing/Pricing";
import { Problem } from "@/components/landing/Problem";
import { ProductDeepDive } from "@/components/landing/ProductDeepDive";
import { WhatIsDotaCoach } from "@/components/landing/WhatIsDotaCoach";
import { LandingBackdrop } from "@/components/shell/LandingBackdrop";
import { SessionHandoff } from "@/components/shell/SessionHandoff";
import type { PlanResponse } from "@/lib/types";

/**
 * The marketing landing page — the only thing an unauthenticated visitor
 * sees, so it is both the pitch and the login. There is no app chrome here:
 * `AppShell` only wraps a signed-in session, so this owns its own nav,
 * sections and footer top to bottom.
 *
 * This is a **server** component, and the reason matters. It used to render
 * only after `SessionProvider` had resolved `/api/auth/me` in the browser,
 * which meant the HTML a crawler received was a dashboard skeleton — the
 * entire pitch existed nowhere in the served document. `page.tsx` now decides
 * server-side (see the sign-in hint cookie) and renders this tree directly,
 * so the landing page is complete in the first byte.
 *
 * Nothing here may become a client component wholesale: `LandingNav` and
 * `Pricing` are the only islands that need to be, and their text still
 * server-renders.
 *
 * Every dollar figure and trial length comes from the backend (`plan`, passed
 * down to `Hero` and `Pricing`) rather than being hard-coded here — the offer
 * is billing configuration, and this page is not the place it is allowed to
 * drift from what checkout actually charges.
 */
export function SignedOut({
  loginError,
  plan,
}: {
  loginError?: string;
  plan?: PlanResponse | null;
}) {
  return (
    <div className="flex min-h-dvh flex-col">
      {/* Renders nothing. Asks once whether this browser already has a
          session, so somebody arriving back from Steam is not stranded on the
          pitch — `/` cannot tell on its own, the session cookie being the
          backend's and `HttpOnly`. */}
      <SessionHandoff />
      <LandingJsonLd
        priceUsd={
          plan ? (plan.plan.amount_cents / 100).toFixed(2) : undefined
        }
        currency={plan?.plan.currency.toUpperCase()}
      />
      <LandingBackdrop />
      <LandingNav />

      <main>
        <Hero loginError={loginError} plan={plan} />
        <Problem />
        <HowItWorks />
        <WhatIsDotaCoach />
        <FeaturesBento />
        <ProductDeepDive />
        <Pricing initialPlan={plan} />
        <FAQ />
        <FinalCta />
      </main>

      <Footer />
    </div>
  );
}
