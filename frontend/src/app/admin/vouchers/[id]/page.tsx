import { VoucherDetail } from "@/components/admin/VoucherDetail";

export const metadata = { title: "Admin · Voucher" };

export default async function AdminVoucherPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  return <VoucherDetail id={id} />;
}
