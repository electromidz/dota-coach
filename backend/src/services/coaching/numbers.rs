//! Checking that every figure the model states is one the backend computed.
//!
//! Citation validation already ensures an insight *points* at real evidence.
//! It does not ensure the insight's prose is true to it — a model can cite
//! `overall.deaths` and then write "you die 7.4 times per 10 minutes" when the
//! evidence says 4.1. The citation is real, the sentence is invented, and the
//! player has no way to tell.
//!
//! So numbers are verified as well as citations: every figure in an insight has
//! to appear in the evidence that insight cites. A figure that does not is not
//! repaired or rounded into place — the insight is dropped, because a coach
//! that is confidently wrong about a number is worse than one that says less.
//!
//! The strictness is only defensible because the prompt is explicit about it:
//! the model is told that every number must come from the evidence, and that
//! advice should be qualitative ("earlier", "before you take the fight") rather
//! than inventing timings the data does not contain.

/// A number as it appeared in text, with the precision it was written at.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Figure {
    value: f64,
    /// Decimal places written. `54.3` has one, `54` has none — which is what
    /// lets a rounded restatement of a measured figure count as the same
    /// figure.
    decimals: u32,
}

/// Pull every number out of a piece of text.
///
/// Deliberately blunt about what a number is: a run of digits, optionally with
/// one decimal point and any thousands separators. Percentages and units come
/// out as bare values, so "55%" and "55" are the same figure — which is right,
/// because the evidence writes percentages both ways.
fn figures(text: &str) -> Vec<Figure> {
    let mut out = Vec::new();
    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }

        // An id like `hero.35` or `benchmark.gold_per_min` is not a claim about
        // a number; skipping the whole token keeps ids from being mistaken for
        // figures the model invented.
        let token_start = bytes[..i]
            .iter()
            .rposition(|c| c.is_whitespace())
            .map_or(0, |p| p + 1);
        let token_is_identifier = bytes[token_start..i].iter().any(|c| *c == '.' || *c == '_')
            && bytes[token_start..i].iter().any(|c| c.is_alphabetic());
        if token_is_identifier {
            while i < bytes.len() && !bytes[i].is_whitespace() {
                i += 1;
            }
            continue;
        }

        let start = i;
        let mut decimals = 0;
        let mut seen_point = false;
        let mut digits = String::new();

        while i < bytes.len() {
            let c = bytes[i];
            if c.is_ascii_digit() {
                digits.push(c);
                if seen_point {
                    decimals += 1;
                }
                i += 1;
            } else if c == ',' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
                // Thousands separator, not a boundary.
                i += 1;
            } else if c == '.'
                && !seen_point
                && i + 1 < bytes.len()
                && bytes[i + 1].is_ascii_digit()
            {
                seen_point = true;
                digits.push('.');
                i += 1;
            } else {
                break;
            }
        }

        if i == start {
            i += 1;
            continue;
        }

        if let Ok(value) = digits.parse::<f64>() {
            out.push(Figure { value, decimals });
        }
    }

    out
}

/// Whether a stated figure is a faithful restatement of a measured one.
///
/// Rounding *down* in precision is allowed — quoting 4.1 when the evidence says
/// 4.14, or 512 when it says 511.8 — because that is how anyone writes a
/// number into a sentence. Rounding up is not: a model writing 4.14 when the
/// evidence only ever said 4.1 has produced precision from nowhere.
fn matches(stated: Figure, measured: Figure) -> bool {
    if (stated.value - measured.value).abs() < f64::EPSILON {
        return true;
    }

    if stated.decimals > measured.decimals {
        return false;
    }

    let factor = 10f64.powi(stated.decimals as i32);
    (measured.value * factor).round() / factor == stated.value
}

/// Every figure in `text` that does not appear in `sources`.
///
/// `sources` is the evidence the claim rests on — the cited statements for an
/// insight, or the whole evidence set for a summary, which is a synthesis of
/// all of it rather than a claim about one item.
pub fn unverifiable(text: &str, sources: &[&str]) -> Vec<f64> {
    let measured: Vec<Figure> = sources.iter().flat_map(|s| figures(s)).collect();

    figures(text)
        .into_iter()
        .filter(|stated| !measured.iter().any(|m| matches(*stated, *m)))
        .map(|f| f.value)
        .collect()
}

/// Every `m:ss` timestamp in `text` that does not appear verbatim in `sources`.
///
/// # Why this exists on top of [`unverifiable`]
///
/// The figure check above splits a timestamp into its parts: `18:42` reads as
/// the two numbers 18 and 42, because a colon is a token boundary. That is fine
/// for arithmetic and useless for timestamps — the evidence for a single match is
/// full of small integers (a death count, a threshold, a percentage), so a
/// fabricated `18:45` will frequently find both of its halves somewhere in the
/// cited text and pass.
///
/// A timestamp is the most convincing thing this pipeline can fabricate. "You
/// died at 18:42 with no defensive cooldown" is indistinguishable, to a reader,
/// from a genuine reading of a replay, and a player who checks it and finds
/// nothing there has been told a confident falsehood about their own game. So
/// timestamps are matched as strings, exactly, against the evidence that was
/// cited — no rounding, no nearest-match, no tolerance.
pub fn unverifiable_clocks(text: &str, sources: &[&str]) -> Vec<String> {
    clocks(text)
        .into_iter()
        .filter(|stated| {
            !sources
                .iter()
                .any(|source| source.contains(stated.as_str()))
        })
        .collect()
}

/// Pull every `m:ss` out of a piece of text.
///
/// One or more digits, a colon, then exactly two digits — the shape every
/// timestamp in the evidence is composed at, by `domain::match_facts::clock`. A
/// stricter reading than the figure scanner uses, because a false positive here
/// would discard honest work.
fn clocks(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }

        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }

        // `m:ss` and nothing looser. Three trailing digits or one are not a
        // clock, and treating them as one would reject text that is fine.
        let has_clock = chars.get(i) == Some(&':')
            && chars.get(i + 1).is_some_and(char::is_ascii_digit)
            && chars.get(i + 2).is_some_and(char::is_ascii_digit)
            && !chars.get(i + 3).is_some_and(char::is_ascii_digit);

        if has_clock {
            out.push(chars[start..i + 3].iter().collect());
            i += 3;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVIDENCE: &[&str] = &[
        "You die 4.14 times per 10 minutes of game time.",
        "Across 30 stored matches, you have won 15 and lost 15 (50%).",
        "On Luna, your gold per minute averages 511.8; the peer median is 558.0 and the top 20% \
         start at 680.0.",
    ];

    #[test]
    fn a_figure_repeated_from_the_evidence_is_verified() {
        assert!(unverifiable("You die 4.14 times per 10 minutes.", EVIDENCE).is_empty());
        assert!(unverifiable("You have won 15 of 30.", EVIDENCE).is_empty());
    }

    #[test]
    fn a_figure_rounded_down_from_the_evidence_is_still_verified() {
        // How a person actually writes a measured number into a sentence.
        assert!(unverifiable("You die about 4.1 times per 10 minutes.", EVIDENCE).is_empty());
        assert!(unverifiable("You average 512 gold per minute.", EVIDENCE).is_empty());
        assert!(unverifiable("You die 4 times per 10 minutes.", EVIDENCE).is_empty());
    }

    #[test]
    fn precision_the_evidence_never_had_is_not_verified() {
        // 4.14 is measured, so 4.144 is the model inventing a decimal place.
        assert_eq!(
            unverifiable("You die 4.144 times per 10 minutes.", EVIDENCE),
            vec![4.144],
        );
    }

    #[test]
    fn an_invented_figure_is_caught() {
        assert_eq!(
            unverifiable("You die 7.4 times per 10 minutes.", EVIDENCE),
            vec![7.4],
        );
        // The classic failure: a plausible target nobody measured.
        assert_eq!(
            unverifiable("Aim for 650 gold per minute by 25 minutes.", EVIDENCE),
            vec![650.0, 25.0],
        );
    }

    #[test]
    fn percentages_and_separators_read_as_the_same_figure() {
        assert!(unverifiable("You win 50% of them.", EVIDENCE).is_empty());

        let with_separator = ["Your net worth averages 24,500."];
        assert!(unverifiable("You finish around 24500 net worth.", &with_separator).is_empty());
    }

    #[test]
    fn an_evidence_id_in_the_prose_is_not_mistaken_for_a_figure() {
        // Ids carry digits; citing one in the text is untidy, not dishonest.
        assert!(unverifiable("As hero.35 shows, you win 50%.", EVIDENCE).is_empty());
    }

    #[test]
    fn prose_with_no_numbers_is_always_verifiable() {
        assert!(unverifiable(
            "Check your buyback before you take a fight, and ward earlier.",
            EVIDENCE,
        )
        .is_empty());
        assert!(unverifiable("", EVIDENCE).is_empty());
    }

    #[test]
    fn a_claim_citing_nothing_can_verify_nothing() {
        assert_eq!(unverifiable("You die 4.14 times.", &[]), vec![4.14]);
        // Except when it states no figure at all.
        assert!(unverifiable("You die too often.", &[]).is_empty());
    }

    const TIMELINE: &[&str] = &[
        "You died 3 times, at 10:12 to Lion, 18:42 to Axe, and 27:04 to Lion.",
        "You completed Blink Dagger at 14:22, Black King Bar at 21:44.",
    ];

    #[test]
    fn a_timestamp_quoted_from_the_evidence_is_verified() {
        assert!(unverifiable_clocks("At 18:42 you took a fight alone.", TIMELINE).is_empty());
        assert!(
            unverifiable_clocks("Your Blink at 14:22 came after the 10:12 death.", TIMELINE)
                .is_empty()
        );
    }

    /// The failure the string match exists for: both halves of `18:45` appear in
    /// the evidence as bare numbers (18 from `18:42`, 45 from nowhere in
    /// particular), so the figure checker can be satisfied by a timestamp that
    /// describes no event in the match.
    #[test]
    fn a_timestamp_the_evidence_never_stated_is_caught() {
        assert_eq!(
            unverifiable_clocks("You died at 18:45 without buyback.", TIMELINE),
            vec!["18:45".to_string()],
        );
        assert_eq!(
            unverifiable_clocks("The fight at 32:10 decided it.", TIMELINE),
            vec!["32:10".to_string()],
        );
    }

    /// No tolerance and no nearest-match. A timestamp one second off describes a
    /// different moment, and "close enough" is how a fabrication gets through.
    #[test]
    fn a_timestamp_near_a_real_one_is_not_close_enough() {
        assert_eq!(
            unverifiable_clocks("At 18:41 you were caught.", TIMELINE),
            vec!["18:41".to_string()],
        );
    }

    #[test]
    fn only_clock_shaped_text_is_read_as_a_timestamp() {
        // Plain numbers, ratios and scorelines are the figure checker's problem,
        // not this one — reading them as clocks would discard honest prose.
        assert!(unverifiable_clocks("You finished 8/3/12 with 612 GPM.", TIMELINE).is_empty());
        assert!(unverifiable_clocks("Around 40% of the game.", TIMELINE).is_empty());
        assert!(unverifiable_clocks("", TIMELINE).is_empty());
        // Three trailing digits are not seconds.
        assert!(unverifiable_clocks("Version 1:234 of the build.", TIMELINE).is_empty());
    }

    #[test]
    fn a_claim_citing_nothing_can_verify_no_timestamp() {
        assert_eq!(
            unverifiable_clocks("You died at 18:42.", &[]),
            vec!["18:42".to_string()],
        );
    }
}
