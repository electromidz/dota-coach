//! Checking a single-match analysis against the numbers before anyone reads it.
//!
//! The model writes the findings; this decides whether each one may stand and
//! how serious it is. Pure functions over the evidence the analysis was
//! written from, so the same stored analysis always renders the same way.
//!
//! Rules:
//!   - Kinds are Strength, Weakness and Key moment. A "warning" is a weakness;
//!     a "recommendation" is dropped (the better play on a finding *is* the
//!     recommendation), and so are kinds that describe a career, not a game.
//!   - A weakness must cite a metric that is below the peer median or worse
//!     than the player's own baseline. One that cites only metrics that are
//!     better on both counts is dropped: the data says it is not a weakness.
//!   - Severity is computed, never taken from the model: a weakness at or
//!     below the 25th percentile is major, any other measured weakness minor.
//!     Unmeasured findings (a moment from the timeline) carry no severity.
//!   - A finding that cites only the result and duration is not about a
//!     decision, so it is dropped.
//!   - A core item completed well after the peer median (`match.item.*`) is
//!     a measured weakness: minor when late, major when ten minutes late.

use crate::domain::coaching::{Evidence, EvidenceFigure, Insight, InsightKind, InsightSeverity};

/// At or below this percentile a measured weakness is major.
pub const MAJOR_AT_OR_BELOW: f32 = 25.0;
/// Below this percentile a metric is behind the peer median.
const MEDIAN: f32 = 50.0;

/// Later than the peer median by more than this is late. The provider buckets
/// in five-minute steps, so finer would be false precision.
pub const LATE_AFTER_SECONDS: f32 = 300.0;
/// Late by this much or more is a major weakness.
pub const MAJOR_LATE_SECONDS: f32 = 600.0;

/// Evidence that says what happened but not what was decided.
const NOT_A_DECISION: [&str; 2] = ["match.result", "match.baseline"];

/// The peer metric a per-match figure is measured against, by figure metric.
fn peer_slug(metric: &str) -> Option<&'static str> {
    match metric {
        "deaths_per_10" => Some("deaths_per_min"),
        "gold_per_min" => Some("gold_per_min"),
        _ => None,
    }
}

/// One cited metric, as the guard reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Reading {
    percentile: Option<f32>,
    worse_than_baseline: bool,
    /// Seconds after the peer median, for a core item completed late.
    late_by: Option<f32>,
}

impl Reading {
    fn is_weak(&self) -> bool {
        self.percentile.is_some_and(|p| p < MEDIAN)
            || self.worse_than_baseline
            || self.late_by.is_some()
    }

    fn is_major(&self) -> bool {
        self.percentile.is_some_and(|p| p <= MAJOR_AT_OR_BELOW)
            || self.late_by.is_some_and(|s| s >= MAJOR_LATE_SECONDS)
    }
}

fn worse(figure: &EvidenceFigure) -> bool {
    figure.baseline.is_some_and(|baseline| {
        if figure.higher_is_better {
            figure.value < baseline
        } else {
            figure.value > baseline
        }
    })
}

fn find<'a>(evidence: &'a [Evidence], id: &str) -> Option<&'a Evidence> {
    evidence.iter().find(|e| e.id == id)
}

/// What the cited evidence says about the metrics behind a finding.
fn readings(insight: &Insight, evidence: &[Evidence]) -> Vec<Reading> {
    insight
        .evidence
        .iter()
        .filter_map(|id| {
            let item = find(evidence, id)?;
            let figure = item.figure.as_ref()?;
            if id.starts_with("match.item.") {
                // Completion time against the peer median; no median, no
                // reading (the timing was not compared).
                let median = figure.median?;
                let behind = figure.value - median;
                return Some(Reading {
                    percentile: None,
                    worse_than_baseline: false,
                    late_by: (behind > LATE_AFTER_SECONDS).then_some(behind),
                });
            }
            if let Some(slug) = id.strip_prefix("match.peer.") {
                // A peer reading; the personal baseline lives on the matching
                // per-match item, when there is one.
                let baseline = evidence.iter().find(|e| {
                    e.figure
                        .as_ref()
                        .and_then(|f| f.metric.as_deref())
                        .and_then(peer_slug)
                        == Some(slug)
                });
                return Some(Reading {
                    percentile: figure.percentile,
                    worse_than_baseline: baseline
                        .and_then(|e| e.figure.as_ref())
                        .is_some_and(worse),
                    late_by: None,
                });
            }
            if !id.starts_with("match.") {
                return None;
            }
            let percentile = figure
                .metric
                .as_deref()
                .and_then(peer_slug)
                .and_then(|slug| find(evidence, &format!("match.peer.{slug}")))
                .and_then(|e| e.figure.as_ref())
                .and_then(|f| f.percentile);
            Some(Reading {
                percentile,
                worse_than_baseline: worse(figure),
                late_by: None,
            })
        })
        .collect()
}

/// Apply the rules to a match analysis's findings, strongest first.
pub fn govern(insights: Vec<Insight>, evidence: &[Evidence]) -> Vec<Insight> {
    let mut kept: Vec<Insight> = insights
        .into_iter()
        .filter_map(|mut insight| {
            insight.kind = match insight.kind {
                InsightKind::Strength | InsightKind::Weakness | InsightKind::KeyMoment => {
                    insight.kind
                }
                InsightKind::Warning => InsightKind::Weakness,
                _ => return None,
            };
            insight.kind_label = insight.kind.label();

            let decision = insight
                .evidence
                .iter()
                .any(|id| !NOT_A_DECISION.contains(&id.as_str()));
            if !decision {
                return None;
            }

            let readings = readings(&insight, evidence);
            insight.severity = None;
            if insight.kind == InsightKind::Weakness && !readings.is_empty() {
                let weak: Vec<&Reading> = readings.iter().filter(|r| r.is_weak()).collect();
                if weak.is_empty() {
                    return None;
                }
                insight.severity = Some(if weak.iter().any(|r| r.is_major()) {
                    InsightSeverity::Major
                } else {
                    InsightSeverity::Minor
                });
            }
            Some(insight)
        })
        .collect();

    let rank = |i: &Insight| match i.severity {
        Some(InsightSeverity::Major) => 0,
        Some(InsightSeverity::Minor) => 1,
        None => 2,
    };
    kept.sort_by_key(rank);
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::benchmark::Confidence;
    use crate::domain::coaching::EvidenceKind;

    fn figure(metric: &str, value: f32, higher: bool, baseline: Option<f32>, pct: Option<f32>) -> EvidenceFigure {
        EvidenceFigure {
            metric: Some(metric.into()),
            hero_id: Some(9),
            value,
            higher_is_better: higher,
            unit: None,
            median: None,
            top_20: None,
            percentile: pct,
            previous: None,
            percent_change: None,
            status: None,
            baseline,
        }
    }

    fn item(id: &str, figure: Option<EvidenceFigure>) -> Evidence {
        Evidence {
            id: id.into(),
            kind: EvidenceKind::Match,
            label: id.into(),
            statement: String::new(),
            sample: 1,
            confidence: Confidence::Insufficient,
            figure,
        }
    }

    fn insight(kind: InsightKind, title: &str, cites: &[&str]) -> Insight {
        Insight {
            kind,
            kind_label: kind.label(),
            title: title.into(),
            explanation: String::new(),
            severity: Some(InsightSeverity::Major),
            timestamp: None,
            what_happened: None,
            why_it_matters: None,
            better_action: Some("Do better".into()),
            evidence: cites.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// The reported match: Mirana, 1.2 deaths per 10 minutes against a
    /// baseline of 1.7 (deaths at the 73rd percentile), GPM at the 97th, and
    /// a BKB at 52:02 in a long game.
    fn mirana_match() -> Vec<Evidence> {
        vec![
            item("match.result", None),
            item("match.deaths", Some(figure("deaths_per_10", 1.2, false, Some(1.7), None))),
            item("match.peer.deaths_per_min", Some(figure("deaths_per_min", 0.12, false, None, Some(73.0)))),
            item("match.economy", Some(figure("gold_per_min", 812.0, true, Some(540.0), None))),
            item("match.peer.gold_per_min", Some(figure("gold_per_min", 812.0, true, None, Some(97.0)))),
            item("match.kda", Some(figure("kda", 5.5, true, Some(4.37), None))),
            item("match.peer.tower_damage", Some(figure("tower_damage", 400.0, true, None, Some(18.0)))),
            item("match.timeline.death.1", None),
        ]
    }

    #[test]
    fn a_major_weakness_the_numbers_contradict_is_dropped() {
        // Deaths at the 73rd percentile and below the baseline: not a weakness.
        let out = govern(
            vec![insight(InsightKind::Weakness, "High death count", &["match.deaths"])],
            &mirana_match(),
        );
        assert!(out.is_empty(), "{out:?}");

        // Citing the peer reading directly makes no difference.
        let out = govern(
            vec![insight(InsightKind::Weakness, "High death count", &["match.peer.deaths_per_min"])],
            &mirana_match(),
        );
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn severity_is_computed_from_the_percentile_gap() {
        let evidence = mirana_match();
        let out = govern(
            vec![insight(InsightKind::Weakness, "Low tower damage", &["match.peer.tower_damage"])],
            &evidence,
        );
        assert_eq!(out[0].severity, Some(InsightSeverity::Major), "18th is a major gap");

        let mut evidence = evidence;
        evidence[6].figure.as_mut().unwrap().percentile = Some(40.0);
        let out = govern(
            vec![insight(InsightKind::Weakness, "Low tower damage", &["match.peer.tower_damage"])],
            &evidence,
        );
        assert_eq!(out[0].severity, Some(InsightSeverity::Minor));
    }

    #[test]
    fn worse_than_your_own_baseline_is_enough_for_a_minor_weakness() {
        let mut evidence = mirana_match();
        // KDA below the player's own average, no peer percentile for KDA.
        evidence[5].figure.as_mut().unwrap().value = 3.0;
        let out = govern(vec![insight(InsightKind::Weakness, "KDA dipped", &["match.kda"])], &evidence);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Some(InsightSeverity::Minor));
    }

    #[test]
    fn strengths_and_moments_carry_no_model_severity() {
        let out = govern(
            vec![
                insight(InsightKind::Strength, "Huge farm", &["match.peer.gold_per_min"]),
                insight(InsightKind::KeyMoment, "Died before Roshan", &["match.timeline.death.1"]),
            ],
            &mirana_match(),
        );
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|i| i.severity.is_none()));
    }

    #[test]
    fn recommendations_and_result_only_findings_are_dropped() {
        let out = govern(
            vec![
                insight(InsightKind::Recommendation, "Buy BKB earlier", &["match.economy"]),
                insight(InsightKind::Weakness, "Long match duration", &["match.result"]),
                insight(InsightKind::Warning, "Fell behind on towers", &["match.peer.tower_damage"]),
            ],
            &mirana_match(),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, InsightKind::Weakness, "a warning is a weakness");
        assert_eq!(out[0].kind_label, "Weakness");
    }

    #[test]
    fn an_unmeasured_weakness_from_the_timeline_is_kept_without_severity() {
        let out = govern(
            vec![insight(InsightKind::Weakness, "Caught out at 31:10", &["match.timeline.death.1"])],
            &mirana_match(),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, None);
    }

    fn timing(seconds: f32, median: Option<f32>) -> Evidence {
        let mut f = figure("item_seconds", seconds, false, None, None);
        f.median = median;
        item("match.item.black_king_bar", Some(f))
    }

    #[test]
    fn a_late_core_item_is_a_weakness_and_ten_minutes_late_is_major() {
        // BKB at 52:02 against a 27:30 peer median.
        let evidence = vec![timing(3122.0, Some(1650.0))];
        let kept = govern(
            vec![insight(InsightKind::Weakness, "Late BKB", &["match.item.black_king_bar"])],
            &evidence,
        );
        assert_eq!(kept[0].severity, Some(InsightSeverity::Major));

        let evidence = vec![timing(2100.0, Some(1650.0))];
        let kept = govern(
            vec![insight(InsightKind::Weakness, "Late BKB", &["match.item.black_king_bar"])],
            &evidence,
        );
        assert_eq!(kept[0].severity, Some(InsightSeverity::Minor));
    }

    #[test]
    fn an_on_time_item_is_not_a_weakness() {
        let evidence = vec![timing(1700.0, Some(1650.0))];
        let kept = govern(
            vec![insight(InsightKind::Weakness, "Late BKB", &["match.item.black_king_bar"])],
            &evidence,
        );
        assert!(kept.is_empty());
    }

    #[test]
    fn an_uncompared_item_is_not_measured() {
        let evidence = vec![timing(3122.0, None)];
        let kept = govern(
            vec![insight(InsightKind::Weakness, "Late BKB", &["match.item.black_king_bar"])],
            &evidence,
        );
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].severity, None);
    }

    #[test]
    fn findings_are_ordered_major_minor_then_the_rest() {
        let mut evidence = mirana_match();
        evidence.push(item("match.peer.hero_damage_per_min", Some(figure("hero_damage_per_min", 300.0, true, None, Some(45.0)))));
        let out = govern(
            vec![
                insight(InsightKind::Strength, "Farm", &["match.peer.gold_per_min"]),
                insight(InsightKind::Weakness, "Damage", &["match.peer.hero_damage_per_min"]),
                insight(InsightKind::Weakness, "Towers", &["match.peer.tower_damage"]),
            ],
            &evidence,
        );
        let titles: Vec<&str> = out.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, vec!["Towers", "Damage", "Farm"]);
    }
}
