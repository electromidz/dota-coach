import { AdminGate } from "@/components/admin/AdminGate";
import { UserDetail } from "@/components/admin/UserDetail";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Admin · Account" };

export default async function AdminUserPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;

  return (
    <AppShell showTabs={false}>
      <AdminGate>
        <UserDetail id={id} />
      </AdminGate>
    </AppShell>
  );
}
