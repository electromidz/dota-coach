import { UserDetail } from "@/components/admin/UserDetail";

export const metadata = { title: "Admin · Account" };

export default async function AdminUserPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  return <UserDetail id={id} />;
}
