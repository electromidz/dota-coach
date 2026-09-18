import { AppBackdrop } from "@/components/shell/AppBackdrop";
import { AppChrome } from "@/components/shell/AppChrome";
import { AppMain } from "@/components/shell/AppMain";
import { SessionProvider } from "@/lib/session-context";

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
        <AppBackdrop />
        <AppChrome title={title} action={action} showTabs={showTabs} />

        <AppMain
          title={title}
          eyebrow={eyebrow}
          description={description}
          action={action}
          showTabs={showTabs}
        >
          {children}
        </AppMain>
      </div>
    </SessionProvider>
  );
}
