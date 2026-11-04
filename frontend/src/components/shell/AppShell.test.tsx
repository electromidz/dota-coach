import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ApiError } from "@/lib/api";

import { AppShell } from "./AppShell";

const getMe = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getMe };
});

vi.mock("next/navigation", () => ({
  usePathname: () => "/rank",
  useRouter: () => ({ push: vi.fn(), replace: vi.fn() }),
  useSearchParams: () => new URLSearchParams(),
}));

describe("AppShell", () => {
  it("keeps the navigation when the profile fails, and retries on request", async () => {
    getMe
      .mockRejectedValueOnce(new ApiError("DATABASE_ERROR", "A database error occurred.", 500, "req-9"))
      .mockResolvedValueOnce({
        user: { persona_name: "Tester", avatar_url: null, is_admin: false },
        dota_player: { rank_tier: 55 },
        matches_stored: 3,
      });

    render(
      <AppShell title="Rank">
        <p>page body</p>
      </AppShell>,
    );

    expect(await screen.findByText("Your profile couldn't load")).toBeDefined();
    expect(screen.getByText("Request req-9")).toBeDefined();
    // Nav and page body still render.
    expect(screen.getAllByRole("link", { name: /Matches/ }).length).toBeGreaterThan(0);
    expect(screen.getByText("page body")).toBeDefined();

    fireEvent.click(screen.getByRole("button", { name: /Retry/ }));
    await waitFor(() => expect(screen.queryByText("Your profile couldn't load")).toBeNull());
    expect(getMe).toHaveBeenCalledTimes(2);
  });
});
