import Link from "next/link";

import { Brand } from "@/components/shell/Brand";
import { Icon, type IconName } from "@/components/ui/Icon";

const COLUMNS: { heading: string; links: { label: string; href: string }[] }[] = [
  {
    heading: "Product",
    links: [
      { label: "Features", href: "#features" },
      { label: "How it works", href: "#how-it-works" },
      { label: "Pricing", href: "#pricing" },
      { label: "FAQ", href: "#faq" },
    ],
  },
  {
    heading: "Company",
    links: [
      { label: "About", href: "#" },
      { label: "Contact", href: "#" },
    ],
  },
  {
    heading: "Legal",
    links: [
      { label: "Terms of service", href: "#" },
      { label: "Privacy policy", href: "#" },
    ],
  },
];

/** Placeholder destinations — see the landing-page notes: wire these to the
 *  real community links before launch. */
const SOCIALS: { icon: IconName; label: string; href: string }[] = [
  { icon: "discord", label: "Discord", href: "#" },
  { icon: "github", label: "GitHub", href: "#" },
  { icon: "x", label: "X", href: "#" },
];

export function Footer() {
  return (
    <footer className="safe-x border-t border-border">
      <div className="mx-auto flex max-w-7xl flex-col gap-10 py-12 lg:flex-row lg:justify-between">
        <div className="flex max-w-xs flex-col gap-3">
          <Brand />
          <p className="text-sm leading-relaxed text-ink-faint">
            A personal AI coach that learns your Dota 2 habits across matches
            and tells you what to train next.
          </p>
          <div className="mt-1 flex items-center gap-3">
            {SOCIALS.map((social) => (
              <a
                key={social.label}
                href={social.href}
                className="focus-neon flex size-9 cursor-pointer items-center justify-center rounded-lg border border-glass-edge text-ink-faint transition-colors duration-200 ease-out hover:text-ink"
              >
                <Icon name={social.icon} title={social.label} className="size-4" />
              </a>
            ))}
          </div>
        </div>

        <div className="grid grid-cols-2 gap-8 sm:grid-cols-3">
          {COLUMNS.map((column) => (
            <div key={column.heading} className="flex flex-col gap-3">
              <p className="text-xs font-semibold uppercase tracking-widest text-ink-faint">
                {column.heading}
              </p>
              <ul className="flex flex-col gap-2">
                {column.links.map((link) => (
                  <li key={link.label}>
                    <Link
                      href={link.href}
                      className="focus-neon rounded text-sm text-ink-muted transition-colors duration-200 ease-out hover:text-ink"
                    >
                      {link.label}
                    </Link>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      </div>

      <div className="mx-auto flex max-w-7xl flex-col gap-2 border-t border-border py-6 text-xs text-ink-faint sm:flex-row sm:items-center sm:justify-between">
        <p>&copy; {new Date().getFullYear()} Dota Coach. All rights reserved.</p>
        <p>Not affiliated with Valve Corporation.</p>
      </div>
    </footer>
  );
}
