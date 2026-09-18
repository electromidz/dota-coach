import { AdminGate } from "@/components/admin/AdminGate";
import { AdminSubNav } from "@/components/admin/AdminSubNav";
import { VoucherTable } from "@/components/admin/VoucherTable";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Admin · Vouchers" };

export default function AdminVouchersPage() {
  return (
    <AppShell
      title="Vouchers"
      eyebrow="Admin"
      description="Create codes that grant subscription time, and see who has redeemed them."
    >
      <div className="flex flex-col gap-6">
        <AdminSubNav />
        <AdminGate>
          <VoucherTable />
        </AdminGate>
      </div>
    </AppShell>
  );
}
