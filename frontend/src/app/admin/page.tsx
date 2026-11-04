import { Suspense } from "react";

import { AdminDashboard } from "@/components/admin/AdminDashboard";
import { DashboardSkeleton } from "@/components/admin/Skeletons";

export const metadata = { title: "Admin" };

export default function AdminPage() {
  // Suspense: the dashboard reads its date range from the URL.
  return (
    <Suspense fallback={<DashboardSkeleton />}>
      <AdminDashboard />
    </Suspense>
  );
}
