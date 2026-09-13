import { cn } from "@/lib/utils";

/**
 * Shared control styling.
 *
 * `min-h-11` is the 44px touch target floor; `cursor-pointer` and a visible
 * focus ring are non-negotiable and therefore live here rather than at each
 * call site.
 */
const BASE = cn(
  "inline-flex min-h-11 cursor-pointer items-center justify-center gap-2",
  // No size utility here on purpose: `--color-base` shadows `text-base`, so
  // writing it would set a near-black colour, not 1rem — which the body
  // already inherits anyway.
  "rounded-xl px-6 font-semibold tracking-wide",
  "transition-[color,background-color,border-color,box-shadow,opacity]",
  "duration-200 ease-out focus-neon",
  "disabled:cursor-not-allowed disabled:opacity-50",
);

const VARIANTS = {
  /* Primary action. Filled with the keyword accent; dark ink on a bright
     neon fill is the only combination here that stays readable — `text-base`
     is the page's near-black, not a font size. */
  primary: cn(
    "bg-accent text-base",
    "shadow-[0_0_28px_-6px_var(--color-keyword)]",
    "hover:brightness-110 hover:shadow-[0_0_36px_-4px_var(--color-keyword)]",
    "disabled:shadow-none",
  ),
  /* Secondary: glass with a cyan edge that lights up on hover. Colour and
     border both change, so the affordance is not carried by hue alone. */
  ghost: cn(
    "glass text-ink",
    "hover:border-function/60 hover:text-function",
  ),
} as const;

export type ButtonVariant = keyof typeof VARIANTS;

export function Button({
  variant = "primary",
  className,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & { variant?: ButtonVariant }) {
  return (
    <button
      type="button"
      {...props}
      className={cn(BASE, VARIANTS[variant], className)}
    />
  );
}

export function ButtonLink({
  variant = "primary",
  className,
  ...props
}: React.AnchorHTMLAttributes<HTMLAnchorElement> & { variant?: ButtonVariant }) {
  return <a {...props} className={cn(BASE, VARIANTS[variant], className)} />;
}
