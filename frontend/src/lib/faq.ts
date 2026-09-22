/**
 * The landing page FAQ, in one place because two things consume it: the
 * `<details>` list a visitor reads, and the `FAQPage` JSON-LD a crawler
 * reads. Structured data that disagrees with the visible answer is a
 * spam signal, so there is deliberately no second copy of this text.
 *
 * Questions are phrased the way people actually search — "is there a free
 * Dota 2 coach", "can an AI coach improve my MMR" — rather than as internal
 * feature names, so the answer text matches the query it is meant to serve.
 */
export interface FaqItem {
  q: string;
  a: string;
}

export const FAQ_ITEMS: FaqItem[] = [
  {
    q: "What is an AI Dota 2 coach?",
    a: "An AI Dota 2 coach reads the matches you have already played, turns them into measurable numbers — laning, deaths, gold, objectives — and tells you which of those numbers is actually holding your MMR back. Dota Coach calculates every figure deterministically on the server and uses the AI only to explain what the numbers mean and what to do about them.",
  },
  {
    q: "Is there a free Dota 2 coach I can try?",
    a: "Yes. Dota Coach starts with a free trial, no card required. Match analysis, rank-scoped benchmarks, hero intelligence and pattern detection stay free after it; the subscription only covers the AI coaching calls that cost money to run.",
  },
  {
    q: "Can a Dota coach actually help me climb MMR?",
    a: "A coach helps when it changes what you do next game rather than describing what already happened. Dota Coach picks a single training focus from repeated evidence across your matches, benchmarks it against players in your own bracket and role, and charts it match over match so you can see whether it is moving.",
  },
  {
    q: "Do I need to install anything?",
    a: "No. Sign in with Steam and the coach reads your public Dota 2 match history through OpenDota — nothing runs on your machine, no overlay, no client mod, no replay uploads.",
  },
  {
    q: "Is this allowed by Valve?",
    a: "Yes. This only reads publicly available match data through OpenDota's API, the same data Dotabuff and Stratz use. It never touches the game client, memory or files, and it never automates play.",
  },
  {
    q: "What Dota 2 data do you actually read?",
    a: "Your public match history: heroes played, KDA, gold and experience, items, and objective participation. We never ask for your Steam password, and Steam's own login never shares it with us.",
  },
  {
    q: "Does it work at low MMR or as a new player?",
    a: "The coach still works — benchmarks are scoped to your bracket, not the pro scene, so a Herald is measured against Heralds. When the sample is too small for a percentile, it says so plainly rather than inventing one.",
  },
  {
    q: "How do I cancel?",
    a: "From your account's billing page, any time. Cancelling stops future charges; every measured feature — stats, benchmarks, hero intelligence, patterns — keeps working, and only new AI analysis is gated.",
  },
  {
    q: "How do voucher codes work?",
    a: "A voucher adds a fixed number of days of subscription access to your account once redeemed. Sign in first, then redeem the code from your billing page — one code, one account.",
  },
];
