import { AdminGate } from "@/components/admin/AdminGate";
import { AdminSubNav } from "@/components/admin/AdminSubNav";
import { UserTable } from "@/components/admin/UserTable";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Admin · Users" };

export default function AdminUsersPage() {
  return (
    <AppShell
      title="Users"
      eyebrow="Admin"
      description="Every account, filterable by status, plan and last seen."
    >
      <div className="flex flex-col gap-6">
        <AdminSubNav />
        <AdminGate>
          <UserTable />
        </AdminGate>
      </div>
    </AppShell>
  );
}
