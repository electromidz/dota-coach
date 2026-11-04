"use client";

import { useState } from "react";

import { Reveal } from "@/components/landing/Reveal";
import { ButtonLink } from "@/components/ui/Button";
import { Icon } from "@/components/ui/Icon";
import { steamLoginUrl } from "@/lib/api";
import { formatPerDay, formatPlanPrice } from "@/lib/billing";
import type { PlanResponse } from "@/lib/types";
import { usePlan } from "@/lib/usePlan";

const TRIAL_FEATURES = [
  "Every Pro feature, nothing held back",
  "No card needed to start",
  "Overview and match history stay free after it",
];

const PRO_FEATURES = [
  "Benchmarks, Rank and next-medal estimate",
  "Hero intelligence and pattern detection",
  "AI coach analyses, chat and training plan",
  "Cancel anytime — you keep Pro until the period ends",
];

/**
 * What each tier can open, mirroring the backend's entitlement gate: the
 * free tier reads `/api/stats` and the match list; everything else needs a
 * trial or a subscription. Keep this in step with the handlers that take
 * `EntitledUser`.
 */
const COMPARISON: { feature: string; free: string | boolean; pro: string | boolean }[] = [
  { feature: "Overview stats and match history", free: true, pro: true },
  { feature: "Match analysis (peer percentiles, findings)", free: false, pro: true },
  { feature: "Benchmarks vs your rank and the next", free: false, pro: true },
  { feature: "Rank and next-medal estimate", free: false, pro: true },
  { feature: "Hero intelligence", free: false, pro: true },
  { feature: "Pattern detection and training focus", free: false, pro: true },
  { feature: "AI coach analyses", free: false, pro: "Daily fair-use cap" },
  { feature: "Coach chat", free: false, pro: true },
  { feature: "Training plan and session progress", free: false, pro: true },
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
          Try everything free{plan ? ` for ${plan.trial_days} days` : ""}. After that your
          overview and match history stay free, and Pro unlocks the rest.
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
            <div>
              <p className="text-sm font-semibold text-keyword">Pro</p>
              <p className="mt-2 font-display text-4xl tracking-wide text-ink">
                {plan ? formatPlanPrice(plan) : " "}
              </p>
              <p className="mt-1 text-sm text-ink-faint">
                {plan ? `About ${formatPerDay(plan)} a day · ` : ""}Paid in crypto
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

            {/* A configuration note for whoever runs this deployment, not
                something to show the public. */}
            {process.env.NODE_ENV !== "production" && !checkoutAvailable && plan ? (
              <p className="text-xs text-ink-faint">
                Dev note: no payment provider is configured — the trial still works fully.
              </p>
            ) : null}
          </div>
        </Reveal>
      </div>

      <Reveal delayMs={150} className="mt-10">
        <Comparison />
      </Reveal>

      <Reveal delayMs={200} className="mt-6">
        <VoucherRedeemHint />
      </Reveal>
    </section>
  );
}

function Comparison() {
  return (
    <div className="glass overflow-x-auto rounded-card">
      <table className="w-full text-sm">
        <caption className="sr-only">What the free tier and Pro include</caption>
        <thead>
          <tr className="border-b border-glass-edge text-left text-xs uppercase tracking-wider text-ink-faint">
            <th scope="col" className="px-3 py-3 font-medium sm:px-4">Feature</th>
            <th scope="col" className="w-12 px-1 py-3 text-center font-medium sm:w-24 sm:px-3">Free</th>
            <th scope="col" className="w-20 px-1 py-3 text-center font-medium text-keyword sm:w-32 sm:px-3">Trial &amp; Pro</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-glass-edge">
          {COMPARISON.map((row) => (
            <tr key={row.feature}>
              <th scope="row" className="px-3 py-2.5 text-left font-normal text-ink-muted sm:px-4">{row.feature}</th>
              <td className="px-1 py-2.5 text-center sm:px-3"><Cell value={row.free} /></td>
              <td className="px-1 py-2.5 text-center sm:px-3"><Cell value={row.pro} /></td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function Cell({ value }: { value: string | boolean }) {
  if (typeof value === "string") return <span className="text-xs text-ink">{value}</span>;
  return value ? (
    <Icon name="check" className="mx-auto size-4 text-string" title="Included" />
  ) : (
    <span className="text-ink-faint">
      <span aria-hidden>—</span>
      <span className="sr-only">Not included</span>
    </span>
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
          className="min-h-10 w-full min-w-0 flex-1 rounded-xl border border-glass-edge bg-surface-2/60 px-3 font-mono text-sm uppercase text-ink outline-none focus-neon"
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
