import { ButtonLink } from "@/components/ui/Button";
import { Icon } from "@/components/ui/Icon";
import { steamLoginUrl } from "@/lib/api";

/**
 * A link, not a fetch: Steam's OpenID flow is a top-level browser redirect and
 * cannot be completed from XHR.
 */
export function SteamLoginButton({ className }: { className?: string }) {
  return (
    <ButtonLink href={steamLoginUrl()} className={className}>
      <Icon name="steam" className="size-5" />
      Sign in with Steam
    </ButtonLink>
  );
}
