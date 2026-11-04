import { AdminGate } from "@/components/admin/AdminGate";
import { AdminSubNav } from "@/components/admin/AdminSubNav";
import { AppShell } from "@/components/shell/AppShell";

/**
 * One frame for every admin screen. The shell, title and tabs mount once and
 * stay put across tab changes; only the page below them is swapped, and each
 * segment's `loading.tsx` covers just that content area.
 *
 * `AdminGate` decides what to *render* for a non-admin. It is not the access
 * control: every `/api/admin/*` endpoint checks `users.is_admin` itself and
 * answers 401/403, so nothing here can be bypassed into data.
 */
export default function AdminLayout({ children }: { children: React.ReactNode }) {
  return (
    <AppShell
      title="Admin"
      eyebrow="Admin"
      description="Who uses the product, what happens during their trial, and who buys."
    >
      <div className="flex flex-col gap-6 pb-4">
        <AdminSubNav />
        <AdminGate>{children}</AdminGate>
      </div>
    </AppShell>
  );
}
