import { Profile } from "@/components/dashboard/Profile";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Profile" };

export default function ProfilePage() {
  return (
    <AppShell
      title="Profile"
      eyebrow="Account"
      description="Your Steam and Dota identities, and what this app has stored."
    >
      <Profile />
    </AppShell>
  );
}
