import { Billing } from "@/components/billing/Billing";
import { AppShell } from "@/components/shell/AppShell";

export const metadata = { title: "Subscription" };

export default function BillingPage() {
  return (
    <AppShell
      title="Subscription"
      eyebrow="Billing"
      description="Your trial, your subscription, and every charge on the account."
    >
      <Billing />
    </AppShell>
  );
}
