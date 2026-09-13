import { Icon } from "@/components/ui/Icon";
import { cn } from "@/lib/utils";

const TONES = {
  error: {
    ring: "neon-ring-error",
    icon: "text-error",
    glyph: "alert",
  },
  success: {
    ring: "neon-ring-string",
    icon: "text-string",
    glyph: "check",
  },
  info: {
    ring: "border-border",
    icon: "text-function",
    glyph: "alert",
  },
} as const;

/**
 * Inline message on a glass panel.
 *
 * Tone is carried by an icon as well as a colour, so it survives a colour
 * vision deficiency and a monochrome screenshot alike.
 */
export function Alert({
  tone = "error",
  title,
  children,
}: {
  tone?: keyof typeof TONES;
  title?: string;
  children: React.ReactNode;
}) {
  const style = TONES[tone];

  return (
    <div
      role={tone === "error" ? "alert" : "status"}
      className={cn(
        "glass flex gap-3 rounded-card p-4 text-sm leading-relaxed",
        style.ring,
      )}
    >
      <Icon
        name={style.glyph}
        className={cn("mt-0.5 size-5 shrink-0", style.icon)}
      />
      <div className="min-w-0">
        {title ? <p className="mb-1 font-semibold text-ink">{title}</p> : null}
        <div className="text-ink-muted">{children}</div>
      </div>
    </div>
  );
}
