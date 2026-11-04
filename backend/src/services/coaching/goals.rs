//! Turning a training plan into measurable goals, and scoring them later.
//!
//! Pure functions over the evidence the analysis was written from. The model
//! chooses *what* to work on (the plan step and the evidence it cites); every
//! target, progress figure and verdict here is arithmetic in Rust.

use crate::domain::coaching::{
    Evidence, GoalOutcome, GoalResult, GoalView, PlanGoal, PlanStep,
};

/// Goals for each plan step that cites a benchmarked metric with a median.
///
/// The target is the next band up: the peer median while below it, then the
/// top-20% line. Already past the top-20% line, the goal is to stay there.
pub fn derive(plan: &[PlanStep], evidence: &[Evidence]) -> Vec<PlanGoal> {
    plan.iter()
        .filter_map(|step| {
            let (item, figure) = step.evidence.iter().find_map(|id| {
                let item = evidence.iter().find(|e| &e.id == id)?;
                let figure = item.figure.as_ref()?;
                (figure.metric.is_some() && figure.median.is_some()).then_some((item, figure))
            })?;

            let median = figure.median?;
            let (target, target_label) =
                if !meets(figure.value, median, figure.higher_is_better) {
                    (median, "Peer median")
                } else {
                    (figure.top_20.unwrap_or(median), "Top 20%")
                };

            Some(PlanGoal {
                position: step.position as i32,
                evidence_id: item.id.clone(),
                metric: figure.metric.clone()?,
                label: item.label.trim_end_matches(" vs peers").to_string(),
                hero_id: figure.hero_id,
                higher_is_better: figure.higher_is_better,
                baseline: figure.value,
                target,
                target_label: target_label.to_string(),
            })
        })
        .collect()
}

/// Whether a value reaches a target, in the direction that is good.
pub fn meets(value: f32, target: f32, higher_is_better: bool) -> bool {
    if higher_is_better {
        value >= target
    } else {
        value <= target
    }
}

/// The value as a share of the target, 0-1, direction honoured.
pub fn progress(value: f32, target: f32, higher_is_better: bool) -> f32 {
    let share = if higher_is_better {
        if target <= 0.0 { 1.0 } else { value / target }
    } else if value <= 0.0 {
        1.0
    } else {
        target / value
    };
    share.clamp(0.0, 1.0)
}

/// A current goal with its progress and recent games.
pub fn view(goal: PlanGoal, current: f32, recent_values: &[f32]) -> GoalView {
    let recent = recent_values
        .iter()
        .map(|v| meets(*v, goal.target, goal.higher_is_better))
        .collect();
    GoalView {
        progress: progress(current, goal.target, goal.higher_is_better),
        current,
        recent,
        goal,
    }
}

/// Score a goal from an earlier analysis against the value now.
pub fn evaluate(goal: PlanGoal, current: Option<f32>) -> GoalResult {
    let outcome = match current {
        Some(now) if meets(now, goal.target, goal.higher_is_better) => GoalOutcome::Hit,
        // Only moving away from the target is a miss. Holding still (often
        // because no new games were played) is still in progress.
        Some(now) => {
            let worse = if goal.higher_is_better {
                now < goal.baseline
            } else {
                now > goal.baseline
            };
            if worse {
                GoalOutcome::Missed
            } else {
                GoalOutcome::InProgress
            }
        }
        None => GoalOutcome::InProgress,
    };
    GoalResult {
        goal,
        current,
        outcome,
    }
}

/// The current value of a metric in a snapshot, by evidence id.
pub fn current_value(evidence: &[Evidence], evidence_id: &str) -> Option<f32> {
    evidence
        .iter()
        .find(|e| e.id == evidence_id)
        .and_then(|e| e.figure.as_ref())
        .map(|f| f.value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::benchmark::Confidence;
    use crate::domain::coaching::{EvidenceFigure, EvidenceKind};

    fn bench(id: &str, value: f32, median: f32, top: f32, higher: bool) -> Evidence {
        Evidence {
            id: id.into(),
            kind: EvidenceKind::Benchmark,
            label: "Tower damage vs peers".into(),
            statement: String::new(),
            sample: 9,
            confidence: Confidence::Low,
            figure: Some(EvidenceFigure {
                metric: Some("tower_damage".into()),
                hero_id: Some(105),
                value,
                higher_is_better: higher,
                unit: None,
                median: Some(median),
                top_20: Some(top),
                percentile: Some(46.0),
                previous: None,
                percent_change: None,
                status: None,
                baseline: None,
            }),
        }
    }

    fn step(position: u32, cites: &[&str]) -> PlanStep {
        PlanStep {
            position,
            title: "t".into(),
            action: "a".into(),
            evidence: cites.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn below_the_median_aims_at_the_median_and_above_it_at_the_top_20() {
        let evidence = [
            bench("benchmark.tower_damage", 300.0, 400.0, 600.0, true),
            bench("benchmark.gold_per_min", 450.0, 400.0, 520.0, true),
        ];
        let goals = derive(
            &[step(1, &["benchmark.tower_damage"]), step(2, &["benchmark.gold_per_min"])],
            &evidence,
        );
        assert_eq!(goals[0].target, 400.0);
        assert_eq!(goals[0].target_label, "Peer median");
        assert_eq!(goals[0].label, "Tower damage");
        assert_eq!(goals[1].target, 520.0);
        assert_eq!(goals[1].target_label, "Top 20%");
    }

    #[test]
    fn a_step_citing_no_benchmark_figure_has_no_goal() {
        let evidence = [Evidence { figure: None, ..bench("overall.deaths", 1.0, 1.0, 1.0, false) }];
        assert!(derive(&[step(1, &["overall.deaths", "missing"])], &evidence).is_empty());
    }

    #[test]
    fn deaths_count_fewer_as_better() {
        assert!(meets(0.15, 0.2, false));
        assert!(!meets(0.25, 0.2, false));
        assert_eq!(progress(0.25, 0.2, false), 0.8);
        assert_eq!(progress(300.0, 400.0, true), 0.75);
        assert_eq!(progress(500.0, 400.0, true), 1.0);
    }

    #[test]
    fn recent_games_are_scored_against_the_target() {
        let goal = derive(&[step(1, &["benchmark.tower_damage"])], &[bench(
            "benchmark.tower_damage",
            300.0,
            400.0,
            600.0,
            true,
        )])
        .remove(0);
        let v = view(goal, 300.0, &[350.0, 410.0, 400.0]);
        assert_eq!(v.recent, vec![false, true, true]);
        assert_eq!(v.progress, 0.75);
    }

    #[test]
    fn an_earlier_goal_is_hit_in_progress_or_missed() {
        let goal = derive(&[step(1, &["benchmark.tower_damage"])], &[bench(
            "benchmark.tower_damage",
            300.0,
            400.0,
            600.0,
            true,
        )])
        .remove(0);
        assert_eq!(evaluate(goal.clone(), Some(420.0)).outcome, GoalOutcome::Hit);
        assert_eq!(evaluate(goal.clone(), Some(350.0)).outcome, GoalOutcome::InProgress);
        assert_eq!(evaluate(goal.clone(), Some(290.0)).outcome, GoalOutcome::Missed);
        // Unchanged, e.g. no games since: not a miss.
        assert_eq!(evaluate(goal.clone(), Some(300.0)).outcome, GoalOutcome::InProgress);
        assert_eq!(evaluate(goal, None).outcome, GoalOutcome::InProgress);
    }
}
