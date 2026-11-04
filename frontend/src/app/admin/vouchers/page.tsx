import { Suspense } from "react";

import { ListSkeleton } from "@/components/admin/Skeletons";
import { VoucherTable } from "@/components/admin/VoucherTable";

export const metadata = { title: "Admin · Vouchers" };

export default function AdminVouchersPage() {
  // Suspense: the status filter and page live in the URL.
  return (
    <Suspense fallback={<ListSkeleton />}>
      <VoucherTable />
    </Suspense>
  );
}
