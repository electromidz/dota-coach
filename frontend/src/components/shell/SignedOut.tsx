import { FAQ } from "@/components/landing/FAQ";
import { FeaturesBento } from "@/components/landing/FeaturesBento";
import { FinalCta } from "@/components/landing/FinalCta";
import { Footer } from "@/components/landing/Footer";
import { Hero } from "@/components/landing/Hero";
import { HowItWorks } from "@/components/landing/HowItWorks";
import { LandingNav } from "@/components/landing/LandingNav";
import { Pricing } from "@/components/landing/Pricing";
import { Problem } from "@/components/landing/Problem";
import { ProductDeepDive } from "@/components/landing/ProductDeepDive";
import { SocialProof } from "@/components/landing/SocialProof";
import { Testimonials } from "@/components/landing/Testimonials";
import { LandingBackdrop } from "@/components/shell/LandingBackdrop";

/**
 * The marketing landing page — the only thing an unauthenticated visitor
 * sees, so it is both the pitch and the login. There is no app chrome here:
 * `AppShell` only wraps a signed-in session, so this owns its own nav,
 * sections and footer top to bottom.
 *
 * Every dollar figure and trial length is read from the backend (`usePlan`,
 * used by `Hero`, `Pricing` and `TrialOffer`) rather than hard-coded here —
 * the offer is billing configuration, and this page is not the place it is
 * allowed to drift from what checkout actually charges.
 */
export function SignedOut({ loginError }: { loginError?: string }) {
  return (
    <div className="flex min-h-dvh flex-col">
      <LandingBackdrop />
      <LandingNav />

      <main>
        <Hero loginError={loginError} />
        <SocialProof />
        <Problem />
        <HowItWorks />
        <FeaturesBento />
        <ProductDeepDive />
        <Testimonials />
        <Pricing />
        <FAQ />
        <FinalCta />
      </main>

      <Footer />
    </div>
  );
}
