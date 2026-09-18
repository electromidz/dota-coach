import { AdminGate } from "@/components/admin/AdminGate";
import { VoucherDetail } from "@/components/admin/VoucherDetail";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Admin · Voucher" };

export default async function AdminVoucherPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;

  return (
    <AppShell showTabs={false}>
      <AdminGate>
        <VoucherDetail id={id} />
      </AdminGate>
    </AppShell>
  );
}
