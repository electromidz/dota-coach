import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { EstablishedRank, Methodology, RankConfidence } from "@/lib/types";

import { MethodologyNote } from "./MethodologyNote";
import { RankCard } from "./RankCard";
import { RolePreferenceBars } from "./RolePreferenceBars";
import { StreakBadge } from "./StreakBadge";

const RANK: EstablishedRank = {
  rank_tier: 45,
  label: "Archon 5",
  leaderboard_rank: null,
  mmr: { low: 2926, high: 3079, midpoint: 3002 },
};

const CONFIDENCE: RankConfidence = {
  confidence_pct: 18,
  matches_counted: 12,
  is_calibrated: false,
};

const METHODOLOGY: Methodology = {
  win_base_mmr: 30,
  loss_base_mmr: 25,
  confidence_per_match_pct: 1.5,
  confidence_threshold_pct: 30,
};

describe("RankCard", () => {
  it("states the medal and how many matches back the confidence", () => {
    render(
      <RankCard rank={RANK} confidence={CONFIDENCE} thresholdPct={30} />,
    );

    expect(screen.getByText("Archon 5")).toBeDefined();
    expect(screen.getByText("18%")).toBeDefined();
    expect(screen.getByText(/12 recent ranked matches/)).toBeDefined();
  });

  /**
   * An account with no medal has no medal. A placeholder shaped like a rank —
   * "Unranked", "Tier 0" — reads as a rank, which is the one thing this screen
   * must not do.
   */
  it("shows no rank rather than a placeholder when none was reported", () => {
    render(
      <RankCard
        rank={{ rank_tier: null, label: null, leaderboard_rank: null, mmr: null }}
        confidence={{ ...CONFIDENCE, matches_counted: 0, confidence_pct: 0 }}
        thresholdPct={30}
      />,
    );

    expect(screen.getByText(/No rank reported/)).toBeDefined();
    expect(screen.queryByText(/Unranked/)).toBeNull();
    expect(screen.queryByText(/Tier 0/)).toBeNull();
  });

  /**
   * An estimate, and labelled as one. The medal is a real reading from Valve;
   * the band is what that reading pins down, and the single figure is the
   * middle of it — not a measurement of where inside the band this player
   * sits, which nothing public can say.
   */
  it("shows the MMR estimate with the band it came from", () => {
    render(<RankCard rank={RANK} confidence={CONFIDENCE} thresholdPct={30} />);

    expect(screen.getByText(/~3,002/)).toBeDefined();
    expect(screen.getByText(/2,926.3,079/)).toBeDefined();
    expect(screen.getByText(/estimated from your medal/)).toBeDefined();
  });

  it("gives Immortal an open-ended band rather than an invented ceiling", () => {
    render(
      <RankCard
        rank={{
          rank_tier: 80,
          label: "Immortal",
          leaderboard_rank: 166,
          mmr: { low: 5421, high: null, midpoint: 5421 },
        }}
        confidence={CONFIDENCE}
        thresholdPct={30}
      />,
    );

    expect(screen.getByText(/5,421\+/)).toBeDefined();
  });

  it("shows no MMR figure when there is no medal to derive one from", () => {
    const { container } = render(
      <RankCard
        rank={{ rank_tier: null, label: null, leaderboard_rank: null, mmr: null }}
        confidence={CONFIDENCE}
        thresholdPct={30}
      />,
    );

    expect(container.textContent).not.toMatch(/MMR/);
  });

  it("marks a calibrated account with a word, not only a colour", () => {
    render(
      <RankCard
        rank={RANK}
        confidence={{ ...CONFIDENCE, confidence_pct: 97, is_calibrated: true }}
        thresholdPct={30}
      />,
    );

    expect(screen.getByText("Calibrated")).toBeDefined();
  });

  it("names the Immortal ladder position when there is one", () => {
    render(
      <RankCard
        rank={{
          rank_tier: 80,
          label: "Immortal",
          leaderboard_rank: 166,
          mmr: { low: 5421, high: null, midpoint: 5421 },
        }}
        confidence={CONFIDENCE}
        thresholdPct={30}
      />,
    );

    expect(screen.getByText("Leaderboard #166")).toBeDefined();
  });
});

describe("StreakBadge", () => {
  it("names the direction in words as well as colour", () => {
    render(<StreakBadge streak={{ count: 4, kind: "loss" }} />);

    expect(screen.getByText("Loss streak")).toBeDefined();
    expect(screen.getByText("4")).toBeDefined();
  });

  /** A zero-length streak has no direction; "Win 0" would be a claim. */
  it("says there is nothing to report rather than rendering a zero streak", () => {
    render(<StreakBadge streak={{ count: 0, kind: null }} />);

    expect(screen.getByText(/No recent ranked matches/)).toBeDefined();
    expect(screen.queryByText("Win streak")).toBeNull();
    expect(screen.queryByText("Loss streak")).toBeNull();
  });
});

describe("RolePreferenceBars", () => {
  it("shows the share and the match count behind it", () => {
    render(
      <RolePreferenceBars
        roles={[{ role: "Offlane", pct: 62.5, matches: 5 }]}
      />,
    );

    expect(screen.getByText("Offlane")).toBeDefined();
    // The count is the bar and the percentage is the annotation, so a 100%
    // resting on two games cannot read as a settled preference.
    expect(screen.getByText(/63%\s*·\s*5/)).toBeDefined();
  });

  it("is empty rather than a zero row without matches", () => {
    render(<RolePreferenceBars roles={[]} />);

    expect(screen.getByText(/No recent ranked matches/)).toBeDefined();
  });
});

describe("MethodologyNote", () => {
  /**
   * The note's whole job is to be true. Every number in it comes from the
   * response, so changing the server's model changes the sentence — a
   * disclosure that has drifted from what it describes is a specific false
   * claim rather than a vague one.
   */
  it("quotes the server's model, not a hardcoded one", () => {
    render(
      <MethodologyNote
        methodology={{
          win_base_mmr: 27,
          loss_base_mmr: 19,
          confidence_per_match_pct: 2,
          confidence_threshold_pct: 42,
        }}
      />,
    );

    expect(screen.getByText("+27")).toBeDefined();
    expect(screen.getByText("−19")).toBeDefined();
    expect(screen.getByText("2%")).toBeDefined();
    expect(screen.getByText("42%")).toBeDefined();

    // And not the defaults it would show if the numbers were baked in.
    expect(screen.queryByText("+30")).toBeNull();
    expect(screen.queryByText("−25")).toBeNull();
  });

  it("says plainly that the dashed points are not Valve's numbers", () => {
    render(<MethodologyNote methodology={METHODOLOGY} />);

    expect(screen.getByText(/modeled/)).toBeDefined();
    expect(
      screen.getByText(/has not published a per-match MMR change/),
    ).toBeDefined();
  });
});
