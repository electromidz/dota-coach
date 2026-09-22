import Link from "next/link";

import { Brand } from "@/components/shell/Brand";
import { Icon, type IconName } from "@/components/ui/Icon";

/**
 * Only destinations that exist. A footer full of `href="#"` costs crawl
 * budget and reads as an abandoned template to a quality rater, so the
 * "Company" and "Legal" columns stay out until there are real pages behind
 * them — see the landing-page notes.
 */
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
    heading: "Learn",
    links: [
      {
        label: "What is an AI Dota 2 coach?",
        href: "#what-is-a-dota-coach",
      },
      { label: "Sample coaching report", href: "#product" },
    ],
  },
];

/** Placeholder destinations — see the landing-page notes: wire these to the
 *  real community links before launch. Until then they are omitted rather
 *  than rendered as dead `#` links. */
const SOCIALS: { icon: IconName; label: string; href: string }[] = [];

export function Footer() {
  return (
    <footer className="safe-x border-t border-border">
      <div className="mx-auto flex max-w-7xl flex-col gap-10 py-12 lg:flex-row lg:justify-between">
        <div className="flex max-w-xs flex-col gap-3">
          <Brand />
          <p className="text-sm leading-relaxed text-ink-faint">
            A personal AI Dota 2 coach that learns your habits across matches,
            benchmarks you against your own rank, and tells you what to train
            next.
          </p>
          {SOCIALS.length ? (
            <div className="mt-1 flex items-center gap-3">
              {SOCIALS.map((social) => (
                <a
                  key={social.label}
                  href={social.href}
                  className="focus-neon flex size-9 cursor-pointer items-center justify-center rounded-lg border border-glass-edge text-ink-faint transition-colors duration-200 ease-out hover:text-ink"
                >
                  <Icon
                    name={social.icon}
                    title={social.label}
                    className="size-4"
                  />
                </a>
              ))}
            </div>
          ) : null}
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
