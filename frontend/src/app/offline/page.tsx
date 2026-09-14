import { Brand } from "@/components/shell/Brand";
import { Alert } from "@/components/ui/Alert";

export const metadata = { title: "Offline" };

/**
 * What the service worker serves when a navigation cannot reach the network.
 *
 * Static by necessity — it is the page that renders when nothing can be
 * fetched — so it makes no claims about the account and shows no data. Its one
 * job is to say which part is broken: the connection, not the coach.
 */
export default function OfflinePage() {
  return (
    <main className="safe-x mx-auto flex min-h-dvh w-full max-w-lg flex-col justify-center gap-6 py-10">
      <Brand />

      <h1 className="font-display text-3xl leading-tight tracking-wide">
        You are offline
      </h1>

      <Alert tone="info" title="Nothing is lost">
        Your matches, metrics and coaching live on the server. Reconnect and
        reload — everything will be where you left it.
      </Alert>

      <p className="text-sm leading-relaxed text-ink-faint">
        This app needs a connection: every number it shows is computed by the
        backend, and it will not invent one from a cache.
      </p>
    </main>
  );
}
