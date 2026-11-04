import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiError } from "@/lib/api";
import type { CoachResponse, Evidence, Insight, PlanStep } from "@/lib/types";

import { MatchAnalysis } from "./MatchAnalysis";

const getMatchAnalysis = vi.hoisted(() => vi.fn());
const analyzeMatch = vi.hoisted(() => vi.fn());
const requestReplayParse = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return { ...actual, getMatchAnalysis, analyzeMatch, requestReplayParse };
});

const ID = "3fa85f64-5717-4562-b3fc-2c963f66afa6";

function evidence(id: string, statement: string): Evidence {
  return {
    id,
    kind: "match",
    label: "Deaths, with times",
    statement,
    sample: 1,
    confidence: "insufficient",
  };
}

const TIMELINE = evidence(
  "match.timeline.deaths",
  "You died 2 times, at 10:00 to Lion, and 10:45 to Lion.",
);

const NO_TIMELINE = evidence(
  "match.timeline.unavailable",
  "No second-by-second timeline exists for this match: STRATZ has it, but the replay was never parsed.",
);

/** The three-part form a single-match insight arrives in. */
function mistake(overrides: Partial<Insight> = {}): Insight {
  return {
    kind: "weakness",
    kind_label: "Weakness",
    title: "You walked back into the fight you had just lost",
    explanation: "",
    severity: "major",
    timestamp: "10:45",
    what_happened: "You respawned and were killed again in the same fight.",
    why_it_matters: "The second death was free for them.",
    better_action: "Take a wave or a camp before you rejoin.",
    evidence: ["match.timeline.deaths"],
    ...overrides,
  };
}

function response(overrides: Partial<CoachResponse> = {}): CoachResponse {
  return {
    role: "carry",
    role_label: "Carry",
    analysis: null,
    evidence: [TIMELINE],
    llm_available: true,
    stale: false,
    cached: false,
    patterns: [],
    note: null,
    ...overrides,
  };
}

function analysed(insights: Insight[], plan: PlanStep[] = []): CoachResponse {
  return response({
    analysis: {
      id: "analysis",
      scope: "match",
      match_id: ID,
      model: "stub-model",
      summary: "You lost this between your first death and your second.",
      insights,
      plan,
      evidence: [TIMELINE],
      generated_at: "2026-09-14T10:00:00Z",
    },
  });
}

afterEach(() => {
  getMatchAnalysis.mockReset();
  analyzeMatch.mockReset();
});

describe("MatchAnalysis", () => {
  it("shows a loading state while the evidence is being read", async () => {
    getMatchAnalysis.mockReturnValue(new Promise(() => {}));

    render(<MatchAnalysis id={ID} />);

    expect(screen.getByText("Reading this match…")).toBeDefined();
  });

  it("offers to analyse before spending a model call, and says what it will read", async () => {
    getMatchAnalysis.mockResolvedValue(response());

    render(<MatchAnalysis id={ID} />);

    expect(await screen.findByRole("button", { name: "Analyse this match" })).toBeDefined();
    // No fabricated analysis in the meantime.
    expect(screen.queryByText("What went wrong?")).toBeNull();
    expect(analyzeMatch).not.toHaveBeenCalled();
  });

  it("shows compact findings with the better play up front and the story behind a toggle", async () => {
    getMatchAnalysis.mockResolvedValue(
      analysed([mistake(), mistake({ title: "Your item came late", severity: "minor", timestamp: null })]),
    );

    render(<MatchAnalysis id={ID} />);

    expect(await screen.findByText("Coach findings")).toBeDefined();

    // Better play is on the card; what happened and why sit in a disclosure.
    expect(screen.getAllByText("Better play")).toHaveLength(4);
    expect(screen.getAllByText("Why it mattered")).toHaveLength(2);
    expect(document.querySelectorAll("details summary").length).toBeGreaterThanOrEqual(2);

    // Severity and the timestamp, both from the backend.
    expect(screen.getByText("major")).toBeDefined();
    expect(screen.getByText("minor")).toBeDefined();
    expect(screen.getByText("10:45")).toBeDefined();

  });

  it("never shows more than three findings", async () => {
    getMatchAnalysis.mockResolvedValue(
      analysed([
        mistake({ title: "One" }),
        mistake({ title: "Two" }),
        mistake({ title: "Three" }),
        mistake({ title: "Four" }),
      ]),
    );
    render(<MatchAnalysis id={ID} />);
    expect(await screen.findByText("Three")).toBeDefined();
    expect(screen.queryByText("Four")).toBeNull();
  });

  it("presents the single training focus as the focus rather than step one of a plan", async () => {
    getMatchAnalysis.mockResolvedValue({
      ...analysed([mistake()]),
      analysis: {
        ...analysed([mistake()]).analysis!,
        plan: [
          {
            position: 1,
            title: "Fight selection",
            action: "Farm one wave after every death before walking to your team.",
            evidence: ["match.timeline.deaths"],
          },
        ],
      },
    });

    render(<MatchAnalysis id={ID} />);

    expect(await screen.findByText("Your main training focus")).toBeDefined();
    expect(screen.getByText("Fight selection")).toBeDefined();
    expect(screen.queryByText("Your training plan")).toBeNull();
  });

  // The most misleading thing this page could do is let a thin reading pass for
  // a complete one.
  it("says up front when no timeline exists, with a way to fix it", async () => {
    getMatchAnalysis.mockResolvedValue(response({ evidence: [NO_TIMELINE] }));
    requestReplayParse.mockResolvedValue({
      requested: true,
      note: "Parse requested. The timeline appears once a parsed replay is available.",
    });

    render(<MatchAnalysis id={ID} />);

    expect(await screen.findByText("No timeline data")).toBeDefined();
    expect(screen.getByRole("link", { name: "How to make your profile public" })).toBeDefined();
    // The reason sits behind the (i); the evidence list still states it.
    expect(screen.getAllByText(/the replay was never parsed/)).toHaveLength(1);

    fireEvent.click(screen.getByRole("button", { name: "Request replay parse" }));
    expect(await screen.findByText(/Parse requested\. The timeline appears/)).toBeDefined();
    expect(requestReplayParse).toHaveBeenCalledWith(ID);
    expect(screen.getByRole("button", { name: "Parse requested" })).toHaveProperty("disabled", true);
  });

  it("does not show the warning for a match that has a timeline", async () => {
    getMatchAnalysis.mockResolvedValue(response());

    render(<MatchAnalysis id={ID} />);

    await screen.findByRole("button", { name: "Analyse this match" });
    expect(screen.queryByText("No timeline data")).toBeNull();
  });

  it("reports a failed load instead of rendering an empty analysis", async () => {
    getMatchAnalysis.mockRejectedValue(
      new ApiError("UPSTREAM_UNAVAILABLE", "Upstream service unavailable: STRATZ", 502),
    );

    render(<MatchAnalysis id={ID} />);

    expect(await screen.findByText("Analysis unavailable")).toBeDefined();
    expect(screen.getByText(/STRATZ/)).toBeDefined();
  });

  it("reports a failed generation as a failure, not as an empty result", async () => {
    getMatchAnalysis.mockResolvedValue(response());
    analyzeMatch.mockRejectedValue(
      new ApiError(
        "UPSTREAM_UNAVAILABLE",
        "Upstream service unavailable: the coaching model",
        502,
      ),
    );

    render(<MatchAnalysis id={ID} />);
    fireEvent.click(
      await screen.findByRole("button", { name: "Analyse this match" }),
    );

    expect(await screen.findByText("The coach could not answer")).toBeDefined();
    // And nothing that looks like a result appeared.
    expect(screen.queryByText("What went wrong?")).toBeNull();
    // The measured evidence is still on the page.
    expect(
      screen.getAllByText(/You died 2 times, at 10:00 to Lion/).length,
    ).toBeGreaterThan(0);
  });

  it("treats a lapsed trial as a state rather than a failure", async () => {
    getMatchAnalysis.mockResolvedValue(response());
    analyzeMatch.mockRejectedValue(
      new ApiError("PAYMENT_REQUIRED", "Your trial has ended.", 402),
    );

    render(<MatchAnalysis id={ID} />);
    fireEvent.click(
      await screen.findByRole("button", { name: "Analyse this match" }),
    );

    expect(await screen.findByText("Your free trial has ended")).toBeDefined();
    expect(screen.queryByText("The coach could not answer")).toBeNull();
  });

  it("shows the analysing state while a generation is in flight", async () => {
    getMatchAnalysis.mockResolvedValue(response());
    analyzeMatch.mockReturnValue(new Promise(() => {}));

    render(<MatchAnalysis id={ID} />);
    fireEvent.click(
      await screen.findByRole("button", { name: "Analyse this match" }),
    );

    await waitFor(() =>
      expect(screen.getByText(/Analysing your match…/)).toBeDefined(),
    );
  });

  it("hides the generate control when no model is configured", async () => {
    getMatchAnalysis.mockResolvedValue(
      response({ llm_available: false, note: "No coaching model is configured." }),
    );

    render(<MatchAnalysis id={ID} />);

    expect(await screen.findByText("No coaching model is configured.")).toBeDefined();
    expect(screen.queryByRole("button", { name: "Analyse this match" })).toBeNull();
  });

  // A stored analysis written before the split form existed uses `explanation`.
  it("still renders the paragraph form of an older analysis", async () => {
    getMatchAnalysis.mockResolvedValue(
      analysed([
        mistake({
          explanation: "You died twice in the same fight.",
          severity: null,
          timestamp: null,
          what_happened: null,
          why_it_matters: null,
          better_action: null,
        }),
      ]),
    );

    render(<MatchAnalysis id={ID} />);

    expect((await screen.findAllByText("You died twice in the same fight.")).length).toBeGreaterThan(0);
    expect(screen.queryByText("What happened")).toBeNull();
  });

  it("offers one compact re-read when the baseline has moved", async () => {
    getMatchAnalysis.mockResolvedValue({ ...analysed([]), stale: true });
    render(<MatchAnalysis id={ID} />);

    expect(await screen.findByText("Your baseline changed since this analysis")).toBeDefined();
    expect(screen.getAllByRole("button", { name: "Re-read" })).toHaveLength(1);
    expect(screen.queryByRole("button", { name: "Analyse this match" })).toBeNull();
  });
});
