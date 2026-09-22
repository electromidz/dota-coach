"use client";

import { useState } from "react";

import { Reveal } from "@/components/landing/Reveal";
import { ButtonLink } from "@/components/ui/Button";
import { Icon } from "@/components/ui/Icon";
import { steamLoginUrl } from "@/lib/api";
import { formatPlanPrice } from "@/lib/billing";
import type { PlanResponse } from "@/lib/types";
import { usePlan } from "@/lib/usePlan";

const TRIAL_FEATURES = [
  "Full match analysis and benchmarks",
  "Hero pool and hero intelligence",
  "Recurring pattern detection",
  "One AI coaching analysis to try it",
];

const PRO_FEATURES = [
  "Everything in the free trial",
  "Unlimited AI coaching analysis",
  "Updated after every synced match",
  "Cancel anytime, no long-term lock-in",
];

/**
 * This product has exactly one paid plan — there is no tiered product line
 * to sell, and no yearly billing the backend can actually charge (`BillingConfig`
 * has one price and one period). So this shows the free trial next to the
 * one real subscription rather than fabricating tiers or a yearly discount
 * nothing behind checkout could honour.
 */
export function Pricing({ initialPlan }: { initialPlan?: PlanResponse | null }) {
  const { plan, checkoutAvailable } = usePlan(initialPlan);

  return (
    <section id="pricing" className="safe-x mx-auto max-w-5xl py-16 lg:py-24">
      <Reveal className="mx-auto max-w-2xl text-center">
        <h2 className="font-display text-3xl tracking-wide sm:text-4xl">
          One plan. No surprises.
        </h2>
        <p className="mt-3 text-ink-muted">
          Dota 2 coaching without a tier ladder: every measured feature is
          free, and the subscription only covers the AI model calls that
          actually cost money to run.
        </p>
      </Reveal>

      <div className="mt-12 grid grid-cols-1 gap-6 sm:grid-cols-2">
        <Reveal>
          <div className="glass flex h-full flex-col gap-6 rounded-card p-6 sm:p-8">
            <div>
              <p className="text-sm font-semibold text-ink-muted">Free trial</p>
              <p className="mt-2 font-display text-4xl tracking-wide text-ink">
                {plan ? `${plan.trial_days} days` : " "}
              </p>
              <p className="mt-1 text-sm text-ink-faint">Then decide</p>
            </div>

            <ul className="flex flex-col gap-3">
              {TRIAL_FEATURES.map((feature) => (
                <FeatureRow key={feature}>{feature}</FeatureRow>
              ))}
            </ul>

            <ButtonLink
              href={steamLoginUrl()}
              variant="ghost"
              className="mt-auto w-full"
            >
              Start free trial
            </ButtonLink>
          </div>
        </Reveal>

        <Reveal delayMs={100}>
          <div className="relative flex h-full flex-col gap-6 rounded-card border border-keyword/50 bg-glass p-6 shadow-[0_0_0_1px_oklch(from_#bb9af7_l_c_h_/_0.35),0_0_40px_-8px_oklch(from_#bb9af7_l_c_h_/_0.55)] sm:p-8">
            <span className="absolute -top-3 left-1/2 -translate-x-1/2 rounded-full bg-keyword px-3 py-1 text-xs font-semibold text-base">
              Most popular
            </span>

            <div>
              <p className="text-sm font-semibold text-keyword">Pro</p>
              <p className="mt-2 font-display text-4xl tracking-wide text-ink">
                {plan ? formatPlanPrice(plan) : " "}
              </p>
              <p className="mt-1 text-sm text-ink-faint">
                Paid in crypto · cancel anytime
              </p>
            </div>

            <ul className="flex flex-col gap-3">
              {PRO_FEATURES.map((feature) => (
                <FeatureRow key={feature}>{feature}</FeatureRow>
              ))}
            </ul>

            <ButtonLink href={steamLoginUrl()} className="mt-auto w-full">
              Start free trial
            </ButtonLink>

            {!checkoutAvailable && plan ? (
              <p className="text-xs text-ink-faint">
                This deployment has no payment provider configured yet — the
                trial still works fully.
              </p>
            ) : null}
          </div>
        </Reveal>
      </div>

      <Reveal delayMs={200} className="mt-6">
        <VoucherRedeemHint />
      </Reveal>
    </section>
  );
}

function FeatureRow({ children }: { children: React.ReactNode }) {
  return (
    <li className="flex items-start gap-2.5 text-sm text-ink-muted">
      <Icon name="check" className="mt-0.5 size-4 shrink-0 text-string" />
      {children}
    </li>
  );
}

/**
 * Redeeming a voucher requires a signed-in account — `/api/subscribe/redeem`
 * has no unauthenticated path, and it shouldn't: subscription time has to
 * land on somebody's account. So this collects the code, then routes the
 * visitor through the real Steam sign-in rather than faking a redemption
 * that would just 401. The signed-in billing page carries the real redeem
 * form once they're through.
 */
function VoucherRedeemHint() {
  const [code, setCode] = useState("");

  return (
    <div className="mx-auto flex w-full max-w-md flex-col items-center gap-2 text-center">
      <p className="text-sm text-ink-faint">Have a voucher code?</p>
      <form
        className="flex w-full items-center gap-2"
        onSubmit={(e) => e.preventDefault()}
      >
        <input
          type="text"
          value={code}
          onChange={(e) => setCode(e.target.value)}
          placeholder="DOTA-XXXX-XXXX"
          aria-label="Voucher code"
          className="min-h-10 flex-1 rounded-xl border border-glass-edge bg-surface-2/60 px-3 font-mono text-sm uppercase text-ink outline-none focus-neon"
        />
        <ButtonLink
          href={steamLoginUrl()}
          variant="ghost"
          className="min-h-10 shrink-0 px-4 text-sm"
        >
          Redeem
        </ButtonLink>
      </form>
      <p className="text-xs text-ink-faint">
        Sign in with Steam first — you&rsquo;ll redeem it from your account.
      </p>
    </div>
  );
}
