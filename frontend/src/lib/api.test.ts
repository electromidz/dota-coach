import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiError, apiFetch } from "./api";

function mockFetch(impl: typeof fetch) {
  vi.stubGlobal("fetch", impl);
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("apiFetch", () => {
  it("returns the parsed body on success", async () => {
    mockFetch(async () => new Response(JSON.stringify({ status: "ok" }), { status: 200 }));

    await expect(apiFetch<{ status: string }>("/health")).resolves.toEqual({
      status: "ok",
    });
  });

  it("surfaces the backend error code and message", async () => {
    mockFetch(
      async () =>
        new Response(
          JSON.stringify({ error: { code: "NOT_FOUND", message: "No such player." } }),
          { status: 404 },
        ),
    );

    await expect(apiFetch("/api/players/1")).rejects.toMatchObject({
      code: "NOT_FOUND",
      message: "No such player.",
      status: 404,
    });
  });

  it("falls back to a generic message when the error body is unparseable", async () => {
    mockFetch(async () => new Response("<html>502</html>", { status: 502 }));

    const error = await apiFetch("/health").catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).code).toBe("UNKNOWN_ERROR");
  });

  it("converts transport failures into a friendly network error", async () => {
    mockFetch(async () => {
      throw new TypeError("fetch failed");
    });

    await expect(apiFetch("/health")).rejects.toMatchObject({
      code: "NETWORK_ERROR",
      status: 0,
    });
  });

  it("sends credentials so the session cookie travels cross-origin", async () => {
    const seen: RequestInit[] = [];
    mockFetch(async (_input, init) => {
      seen.push(init as RequestInit);
      return new Response("{}", { status: 200 });
    });

    await apiFetch("/api/players/me");

    expect(seen[0]?.credentials).toBe("include");
  });

  it("flags an unauthenticated response so the UI can show the signed-out state", async () => {
    mockFetch(
      async () =>
        new Response(
          JSON.stringify({
            error: { code: "UNAUTHENTICATED", message: "Sign in with Steam to continue." },
          }),
          { status: 401 },
        ),
    );

    const error = (await apiFetch("/api/players/me").catch((e: unknown) => e)) as ApiError;

    expect(error.isUnauthenticated).toBe(true);
    expect(error.status).toBe(401);
  });

  it("does not treat other errors as unauthenticated", async () => {
    mockFetch(
      async () =>
        new Response(
          JSON.stringify({ error: { code: "NOT_FOUND", message: "Match not found." } }),
          { status: 404 },
        ),
    );

    const error = (await apiFetch("/api/matches/x").catch((e: unknown) => e)) as ApiError;

    expect(error.isUnauthenticated).toBe(false);
  });
});
