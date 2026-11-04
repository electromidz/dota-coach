import { afterEach, describe, expect, it, vi } from "vitest";

import {
  ApiError,
  apiFetch,
  getBenchmark,
  getMatches,
  selectCoachingRole,
} from "./api";

function mockFetch(impl: typeof fetch) {
  vi.stubGlobal("fetch", impl);
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("apiFetch", () => {
  it("carries the server's request id on an error", async () => {
    mockFetch(
      async () =>
        new Response(JSON.stringify({ error: { code: "DATABASE_ERROR", message: "x" } }), {
          status: 500,
          headers: { "x-request-id": "req-123" },
        }),
    );
    await expect(apiFetch("/api/coach")).rejects.toMatchObject({ requestId: "req-123" });
  });

  it("retries a read once after a dropped connection, never a write", async () => {
    let calls = 0;
    mockFetch(async () => {
      calls += 1;
      if (calls === 1) throw new TypeError("network");
      return new Response(JSON.stringify({ ok: true }), { status: 200 });
    });
    await expect(apiFetch("/api/stats")).resolves.toEqual({ ok: true });
    expect(calls).toBe(2);

    calls = 0;
    await expect(apiFetch("/api/players/me/sync", { method: "POST" })).rejects.toMatchObject({
      code: "NETWORK_ERROR",
    });
    expect(calls).toBe(1);
  });

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

describe("getMatches", () => {
  /** Captures the URL the client actually requested. */
  function captureUrl(): { current: string } {
    const captured = { current: "" };
    mockFetch(async (input: RequestInfo | URL) => {
      captured.current = String(input);
      return new Response(JSON.stringify({ matches: [], page: 1, limit: 20 }), {
        status: 200,
      });
    });
    return captured;
  }

  it("asks for Ranked + All Pick by default, with no mode parameter", async () => {
    const url = captureUrl();
    await getMatches();

    expect(url.current).toContain("/api/matches?page=1&limit=20");
    expect(url.current).not.toContain("mode=");
    expect(url.current).not.toContain("scope=");
  });

  it("asks for Turbo or everything only when told to", async () => {
    const url = captureUrl();
    await getMatches(1, 20, { mode: "turbo" });
    expect(url.current).toContain("mode=turbo");
    await getMatches(1, 20, { mode: "all" });
    expect(url.current).toContain("mode=all");
  });
});

describe("selectCoachingRole", () => {
  it("posts the chosen role slug", async () => {
    let seen: { url: string; init?: RequestInit } = { url: "" };
    mockFetch(async (input: RequestInfo | URL, init?: RequestInit) => {
      seen = { url: String(input), init };
      return new Response(JSON.stringify({ profile: null }), { status: 200 });
    });

    await selectCoachingRole("hard_support");

    expect(seen.url).toContain("/api/coach/role");
    expect(seen.init?.method).toBe("POST");
    // The slug travels verbatim: the backend rejects anything that is not one
    // of the five, so a label ("Hard Support") here would be a 400.
    expect(seen.init?.body).toBe(JSON.stringify({ role: "hard_support" }));
  });
});

describe("getBenchmark", () => {
  function captureUrl(): { current: string } {
    const captured = { current: "" };
    mockFetch(async (input: RequestInfo | URL) => {
      captured.current = String(input);
      return new Response(JSON.stringify({ results: [] }), { status: 200 });
    });
    return captured;
  }

  it("sends no scope by default, so the backend follows the coaching role", async () => {
    const url = captureUrl();
    await getBenchmark();

    expect(url.current).toContain("/api/benchmark");
    expect(url.current).not.toContain("role=");
    expect(url.current).not.toContain("hero_id=");
  });

  it("can name a hero and widen the scope to every role", async () => {
    const url = captureUrl();
    await getBenchmark(35, "all");

    expect(url.current).toContain("hero_id=35");
    expect(url.current).toContain("role=all");
  });
});
