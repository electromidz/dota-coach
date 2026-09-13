import { AppChrome } from "@/components/shell/AppChrome";
import { PageHeader } from "@/components/shell/PageHeader";
import { SessionProvider } from "@/lib/session-context";
import { cn } from "@/lib/utils";

/**
 * The application frame: mobile-first, with a real desktop layout on top.
 *
 * Phones get the native pattern the product was designed around — a compact
 * blurred title strip pinned to the top, one scrolling column, and a fixed
 * bottom tab bar.
 *
 * From `lg` up the same screens become a desktop app rather than a stretched
 * phone: the destinations move into a header bar, the column widens to
 * `max-w-7xl` so the dashboard grids can use the space, and the title grows
 * into a header block with its action beside it.
 */
export function AppShell({
  title,
  eyebrow,
  description,
  action,
  showTabs = true,
  children,
}: {
  title?: React.ReactNode;
  eyebrow?: string;
  description?: string;
  action?: React.ReactNode;
  showTabs?: boolean;
  children: React.ReactNode;
}) {
  return (
    <SessionProvider>
      <div className="min-h-dvh">
        <AppChrome title={title} action={action} showTabs={showTabs} />

        <main
          className={cn(
            "safe-x mx-auto w-full max-w-lg pt-5 lg:max-w-7xl lg:pt-8",
            showTabs ? "app-scroll" : "pb-10",
          )}
        >
          {title ? (
            <PageHeader
              title={title}
              eyebrow={eyebrow}
              description={description}
              action={action}
            />
          ) : null}

          {children}
        </main>
      </div>
    </SessionProvider>
  );
}
