import { Suspense } from "react";

import { ListSkeleton } from "@/components/admin/Skeletons";
import { UserTable } from "@/components/admin/UserTable";

export const metadata = { title: "Admin · Users" };

export default function AdminUsersPage() {
  // Suspense: filters, sort and page live in the URL.
  return (
    <Suspense fallback={<ListSkeleton />}>
      <UserTable />
    </Suspense>
  );
}
