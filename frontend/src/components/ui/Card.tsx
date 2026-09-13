import { cn } from "@/lib/utils";

/**
 * Glass surface: blurred translucent panel over the page's ambient glow.
 *
 * `glow` tints the edge with a syntax token colour for cards that carry state
 * (a win, a failure). Left off, the card stays neutral so a screen full of
 * them does not turn into a light show.
 */
export function Card({
  className,
  glow,
  children,
}: {
  className?: string;
  glow?: "keyword" | "function" | "string" | "number" | "error";
  children: React.ReactNode;
}) {
  return (
    <div
      className={cn(
        "glass rounded-card p-5 sm:p-6",
        glow && `neon-ring-${glow}`,
        className,
      )}
    >
      {children}
    </div>
  );
}
