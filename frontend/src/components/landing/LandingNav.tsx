"use client";

import { useEffect, useState } from "react";

import { Brand } from "@/components/shell/Brand";
import { ButtonLink } from "@/components/ui/Button";
import { Icon } from "@/components/ui/Icon";
import { steamLoginUrl } from "@/lib/api";
import { cn } from "@/lib/utils";

const LINKS = [
  { href: "#features", label: "Features" },
  { href: "#how-it-works", label: "How it works" },
  { href: "#pricing", label: "Pricing" },
  { href: "#faq", label: "FAQ" },
];

/**
 * Sticky top nav. Transparent over the hero so the ambient glow shows
 * through; once the page has scrolled past the hero it picks up the same
 * `glass` treatment as every other panel, so it stops floating over content
 * unreadably.
 */
export function LandingNav() {
  const [scrolled, setScrolled] = useState(false);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 40);
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  useEffect(() => {
    if (!open) return;

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("keydown", onKey);
    document.body.style.overflow = "hidden";

    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = "";
    };
  }, [open]);

  return (
    <header
      className={cn(
        "safe-top sticky top-0 z-40 border-b transition-[background-color,backdrop-filter,border-color] duration-300",
        scrolled
          ? "glass rounded-none border-x-0 border-t-0"
          : "border-transparent bg-transparent",
      )}
    >
      <div className="safe-x mx-auto flex h-16 max-w-7xl items-center justify-between gap-6">
        <Brand />

        <nav aria-label="Primary" className="hidden lg:block">
          <ul className="flex items-center gap-1">
            {LINKS.map((link) => (
              <li key={link.href}>
                <a
                  href={link.href}
                  className="focus-neon block cursor-pointer rounded-xl px-3.5 py-2 text-sm font-medium tracking-wide text-ink-muted transition-colors duration-200 ease-out hover:text-ink"
                >
                  {link.label}
                </a>
              </li>
            ))}
          </ul>
        </nav>

        <div className="hidden items-center gap-3 lg:flex">
          <ButtonLink
            href={steamLoginUrl()}
            variant="ghost"
            className="min-h-9 px-4 text-sm"
          >
            Sign in
          </ButtonLink>
          <ButtonLink
            href={steamLoginUrl()}
            variant="primary"
            className="min-h-9 px-4 text-sm"
          >
            Start free trial
          </ButtonLink>
        </div>

        <button
          type="button"
          onClick={() => setOpen(true)}
          aria-expanded={open}
          aria-controls="landing-mobile-nav"
          className="focus-neon flex size-10 cursor-pointer items-center justify-center rounded-xl text-ink lg:hidden"
        >
          <Icon name="menu" title="Open menu" className="size-6" />
        </button>
      </div>

      {open ? (
        <div
          id="landing-mobile-nav"
          role="dialog"
          aria-modal="true"
          aria-label="Menu"
          className="safe-top safe-x fixed inset-0 z-50 flex flex-col gap-8 bg-base/98 pt-5 pb-10 lg:hidden"
        >
          <div className="flex h-11 items-center justify-between">
            <Brand />
            <button
              type="button"
              onClick={() => setOpen(false)}
              className="focus-neon flex size-10 cursor-pointer items-center justify-center rounded-xl text-ink"
            >
              <Icon name="close" title="Close menu" className="size-6" />
            </button>
          </div>

          <nav aria-label="Primary">
            <ul className="flex flex-col gap-1">
              {LINKS.map((link) => (
                <li key={link.href}>
                  <a
                    href={link.href}
                    onClick={() => setOpen(false)}
                    className="focus-neon block cursor-pointer rounded-xl px-2 py-3 text-lg font-medium text-ink"
                  >
                    {link.label}
                  </a>
                </li>
              ))}
            </ul>
          </nav>

          <div className="mt-auto flex flex-col gap-3">
            <ButtonLink href={steamLoginUrl()} variant="ghost" className="w-full">
              Sign in
            </ButtonLink>
            <ButtonLink href={steamLoginUrl()} variant="primary" className="w-full">
              Start free trial
            </ButtonLink>
          </div>
        </div>
      ) : null}
    </header>
  );
}
