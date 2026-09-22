import { render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ApiError } from "@/lib/api";

import { SessionHandoff } from "./SessionHandoff";

const getMe = vi.hoisted(() => vi.fn());
const refresh = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getMe };
});

vi.mock("next/navigation", () => ({
  useRouter: () => ({ refresh }),
}));

function hint(): string | undefined {
  return document.cookie
    .split("; ")
    .find((c) => c.startsWith("dc_signed_in="))
    ?.split("=")[1];
}

beforeEach(() => {
  document.cookie = "dc_signed_in=; Path=/; Max-Age=0";
});

afterEach(() => {
  getMe.mockReset();
  refresh.mockReset();
});

describe("SessionHandoff", () => {
  /**
   * The regression this exists for. `/` picks its branch server-side from the
   * `dc_signed_in` cookie, and that cookie used to be written only by
   * `SessionProvider` — which only mounts on the branch the cookie unlocks.
   * Nothing could write the first one, so a visitor returning from a
   * successful Steam login stayed on the marketing page permanently.
   */
  it("writes the hint and re-renders when the visitor already has a session", async () => {
    getMe.mockResolvedValue({ steam_id: "1", persona_name: "Test" });

    render(<SessionHandoff />);

    await waitFor(() => expect(hint()).toBe("1"));
    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it("leaves an anonymous visitor on the page they asked for", async () => {
    getMe.mockRejectedValue(new ApiError("UNAUTHENTICATED", "No session.", 401));

    render(<SessionHandoff />);

    await waitFor(() => expect(getMe).toHaveBeenCalled());
    expect(refresh).not.toHaveBeenCalled();
    expect(hint()).toBeUndefined();
  });

  /** A hint outlasting its session would bounce the visitor to a dashboard
   *  that only 401s; a confirmed 401 clears it. */
  it("clears a hint left behind by an expired session", async () => {
    document.cookie = "dc_signed_in=1; Path=/";
    getMe.mockRejectedValue(new ApiError("UNAUTHENTICATED", "No session.", 401));

    render(<SessionHandoff />);

    await waitFor(() => expect(hint()).toBeUndefined());
    expect(refresh).not.toHaveBeenCalled();
  });

  /**
   * An outage says nothing about whether the visitor is signed in. Clearing
   * the hint on a 500 would log a paying user out of their own layout every
   * time the backend hiccuped.
   */
  it("leaves the hint alone when the service is unreachable", async () => {
    document.cookie = "dc_signed_in=1; Path=/";
    getMe.mockRejectedValue(new ApiError("INTERNAL", "Database down.", 500));

    render(<SessionHandoff />);

    await waitFor(() => expect(getMe).toHaveBeenCalled());
    expect(hint()).toBe("1");
    expect(refresh).not.toHaveBeenCalled();
  });
});
