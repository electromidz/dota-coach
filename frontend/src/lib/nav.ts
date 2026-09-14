import type { IconName } from "@/components/ui/Icon";

export interface NavItem {
  href: string;
  label: string;
  icon: IconName;
}

/**
 * The app's primary destinations.
 *
 * One list, two renderings: a bottom tab bar on phones and a horizontal bar in
 * the desktop header. Keeping them in sync by construction means a new section
 * cannot appear on one breakpoint and go missing on the other.
 */
export const NAV_ITEMS: NavItem[] = [
  { href: "/", label: "Overview", icon: "home" },
  { href: "/matches", label: "Matches", icon: "swords" },
  { href: "/benchmark", label: "Benchmark", icon: "gauge" },
  { href: "/heroes", label: "Heroes", icon: "spark" },
  { href: "/coach", label: "Coach", icon: "trophy" },
  { href: "/profile", label: "Profile", icon: "user" },
];

/** "/" must match exactly, or it would light up on every route. */
export function isActiveRoute(pathname: string, href: string): boolean {
  return href === "/" ? pathname === "/" : pathname.startsWith(href);
}
