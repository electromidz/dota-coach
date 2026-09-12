import { cn } from "@/lib/utils";

export function Card({
  className,
  children,
}: {
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <div
      className={cn(
        "rounded-card border border-border bg-surface p-5 sm:p-6",
        className,
      )}
    >
      {children}
    </div>
  );
}
