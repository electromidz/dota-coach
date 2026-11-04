import { Icon, type IconName } from "@/components/ui/Icon";

const ITEMS: { icon: IconName; text: string }[] = [
  { icon: "steam", text: "Steam OpenID sign-in — we never see a password" },
  { icon: "shield", text: "Read-only public match data via OpenDota" },
  { icon: "gauge", text: "Rank and MMR are estimates, not official MMR" },
  { icon: "info", text: "Not affiliated with Valve" },
];

/** What signing in does and does not involve, in one row. */
export function TrustStrip() {
  return (
    <section aria-label="How your data is handled" className="safe-x mx-auto max-w-7xl">
      <ul className="glass flex flex-col gap-3 rounded-card px-5 py-4 text-sm text-ink-muted sm:flex-row sm:flex-wrap sm:justify-center sm:gap-x-8">
        {ITEMS.map((item) => (
          <li key={item.text} className="flex items-center gap-2">
            <Icon name={item.icon} className="size-4 shrink-0 text-function" />
            {item.text}
          </li>
        ))}
      </ul>
    </section>
  );
}
