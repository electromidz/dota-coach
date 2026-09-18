import { AdminDashboard } from "@/components/admin/AdminDashboard";
import { AdminGate } from "@/components/admin/AdminGate";
import { AdminSubNav } from "@/components/admin/AdminSubNav";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Admin" };

export default function AdminPage() {
  return (
    <AppShell
      title="Admin"
      eyebrow="Dashboard"
      description="How many people use the product, what happens during their trial, and how many buy."
    >
      <div className="flex flex-col gap-6">
        <AdminSubNav />
        <AdminGate>
          <AdminDashboard />
        </AdminGate>
      </div>
    </AppShell>
  );
}
