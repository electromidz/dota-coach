import { TabBarGate } from "@/components/shell/TabBarGate";
import { SessionProvider } from "@/lib/session-context";
import { cn } from "@/lib/utils";

/**
 * Native-style app shell: a compact blurred header pinned to the top, a
 * scrolling content column, and a fixed tab bar.
 *
 * Mobile-first — the column is capped at `max-w-lg` so the layout stays a
 * phone-width app on a desktop monitor rather than stretching into a web page.
 */
export function AppShell({
  title,
  action,
  showTabs = true,
  children,
}: {
  title?: React.ReactNode;
  action?: React.ReactNode;
  showTabs?: boolean;
  children: React.ReactNode;
}) {
  return (
    <SessionProvider>
      <div className="min-h-dvh">
        {title ? (
          <header className="glass safe-top sticky top-0 z-20 rounded-none border-x-0 border-t-0">
            <div className="safe-x mx-auto flex h-14 max-w-lg items-center justify-between gap-3">
              <h1 className="truncate font-display text-lg tracking-wide">
                {title}
              </h1>
              {action}
            </div>
          </header>
        ) : null}

        <main
          className={cn(
            "safe-x mx-auto w-full max-w-lg pt-5",
            showTabs ? "app-scroll" : "pb-10",
          )}
        >
          {children}
        </main>

        {showTabs ? <TabBarGate /> : null}
      </div>
    </SessionProvider>
  );
}
