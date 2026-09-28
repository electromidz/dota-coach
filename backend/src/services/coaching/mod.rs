//! The AI coach.
//!
//! The pipeline is deliberately one-directional:
//!
//! ```text
//! deterministic evidence ──► prompt ──► model ──► validation ──► storage
//! ```
//!
//! Nothing the model returns is trusted on the way back. An insight survives
//! only if its kind is one of the six the domain defines and every evidence id
//! it cites exists in the evidence that was sent. That is the mechanism behind
//! "the LLM is an interpretation layer, not a source of truth" — not a comment
//! asking it to behave.

pub mod chat;
pub mod evidence;
pub mod numbers;
pub mod prompt;

use std::collections::HashSet;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::config::CoachConfig;
use crate::domain::coaching::{
    AnalysisScope, Evidence, Insight, InsightKind, InsightSeverity, PlanStep,
};
use crate::services::llm::{LlmError, LlmProvider, LlmRequest};

#[derive(Debug, thiserror::Error)]
pub enum CoachingError {
    #[error(transparent)]
    Llm(#[from] LlmError),
    /// The model answered, but nothing in the answer survived validation.
    #[error("the coaching model returned an unusable analysis: {0}")]
    Unusable(String),
    /// Nothing to analyse. Not a failure — there is simply no history yet.
    #[error("no evidence to analyse")]
    NoEvidence,
}

impl CoachingError {
    pub fn user_note(&self) -> String {
        match self {
            CoachingError::Llm(e) => e.user_note().to_string(),
            CoachingError::Unusable(_) => {
                "The coaching model's answer could not be verified against your data, so it was discarded.".into()
            }
            CoachingError::NoEvidence => {
                "Sync some matches first — there is nothing to analyse yet.".into()
            }
        }
    }
}

/// A validated analysis, before it is stored.
pub struct Generated {
    pub summary: String,
    pub insights: Vec<Insight>,
    pub plan: Vec<PlanStep>,
    pub model: String,
}

/// Ask the model, then verify what comes back.
pub async fn generate(
    llm: &dyn LlmProvider,
    scope: AnalysisScope,
    evidence: &[Evidence],
    config: &CoachConfig,
) -> Result<Generated, CoachingError> {
    if evidence.is_empty() {
        return Err(CoachingError::NoEvidence);
    }

    let plan_steps = plan_cap(scope, config.max_plan_steps);

    let request = LlmRequest {
        system: prompt::system(scope, config.max_insights, plan_steps),
        user: prompt::user(scope, evidence),
        // A structured analysis is one question, not a conversation.
        history: Vec::new(),
        max_output_tokens: config.max_output_tokens,
        temperature: config.temperature,
        json_only: true,
    };

    let completion = llm.generate(&request).await?;
    let draft = parse_analysis(&completion.text, evidence, config.max_insights, plan_steps)?;

    Ok(Generated {
        summary: draft.summary,
        insights: draft.insights,
        plan: draft.plan,
        model: completion.model,
    })
}

/// How many plan steps a scope is allowed to produce.
///
/// One for a single match, and that is the whole feature: the output of reading
/// one game is **one** thing to work on next. A player handed four things to
/// change from a forty-minute game changes none of them, and the specification
/// is explicit that the match analysis ends in a single primary training focus.
///
/// A career or role analysis is a different question — it covers many games and a
/// staged plan is the right answer there — so it keeps the configured cap.
pub fn plan_cap(scope: AnalysisScope, configured: usize) -> usize {
    match scope {
        AnalysisScope::Match => 1,
        AnalysisScope::Player | AnalysisScope::Role => configured,
    }
}

#[derive(Debug)]
pub struct Draft {
    pub summary: String,
    pub insights: Vec<Insight>,
    pub plan: Vec<PlanStep>,
}

/// What the model is expected to send. Every field is optional or lenient,
/// because a strict deserialize would turn one unexpected key into a total
/// failure; the strictness lives in the validation below.
#[derive(Debug, Deserialize)]
struct RawAnalysis {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    insights: Vec<RawInsight>,
    /// Accepts the key the prompt asks for plus the shape models substitute
    /// for it most often.
    #[serde(default, alias = "training_plan")]
    plan: Vec<RawPlanStep>,
}

#[derive(Debug, Deserialize)]
struct RawPlanStep {
    #[serde(default)]
    title: String,
    /// `action` is what the prompt asks for; the other two are what models
    /// write when they forget.
    #[serde(default, alias = "explanation", alias = "detail")]
    action: String,
    #[serde(default, alias = "evidence_ids", alias = "evidence_id")]
    evidence: EvidenceRefs,
}

#[derive(Debug, Deserialize)]
struct RawInsight {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    explanation: String,
    /// `major` or `minor`. Anything else is discarded rather than coerced.
    #[serde(default)]
    severity: String,
    /// `m:ss`, and only kept if the cited evidence contains it verbatim.
    #[serde(default, alias = "time", alias = "at")]
    timestamp: String,
    /// The three-part form single-match analysis asks for. `camelCase` aliases
    /// throughout: the keys are snake_case to match the rest of the API, and a
    /// model reading a JSON example will still reach for camelCase often enough
    /// to be worth accepting.
    #[serde(default, alias = "whatHappened")]
    what_happened: String,
    #[serde(default, alias = "whyItMatters")]
    why_it_matters: String,
    #[serde(default, alias = "betterAction", alias = "better_play")]
    better_action: String,
    /// Accepts the key the prompt asks for, plus the two shapes models most
    /// often substitute for it.
    #[serde(default, alias = "evidence_ids", alias = "evidence_id")]
    evidence: EvidenceRefs,
}

/// One id or several. A model that cites a single evidence item frequently
/// sends a bare string rather than a one-element array.
#[derive(Debug, Default, Deserialize)]
#[serde(untagged)]
enum EvidenceRefs {
    #[default]
    None,
    One(String),
    Many(Vec<String>),
}

impl EvidenceRefs {
    fn into_vec(self) -> Vec<String> {
        match self {
            EvidenceRefs::None => Vec::new(),
            EvidenceRefs::One(id) => vec![id],
            EvidenceRefs::Many(ids) => ids,
        }
    }
}

/// Validate a model answer against the evidence it was given.
///
/// Drops rather than repairs: an insight with an unknown kind or with no
/// surviving citation is discarded, because the alternative — guessing what
/// was meant — is how unverified claims reach a user.
pub fn parse_analysis(
    raw: &str,
    evidence: &[Evidence],
    max_insights: usize,
    max_plan_steps: usize,
) -> Result<Draft, CoachingError> {
    let json = extract_json(raw)
        .ok_or_else(|| CoachingError::Unusable("the answer contained no JSON object".into()))?;

    let parsed: RawAnalysis = serde_json::from_str(json)
        .map_err(|e| CoachingError::Unusable(format!("unparseable JSON: {e}")))?;

    let known: HashSet<&str> = evidence.iter().map(|e| e.id.as_str()).collect();

    let insights: Vec<Insight> = parsed
        .insights
        .into_iter()
        .filter_map(|raw| {
            let kind = InsightKind::parse(&raw.kind)?;

            let refs = cited(raw.evidence, &known);

            // The rule the whole phase rests on: no evidence, no insight.
            if refs.is_empty() {
                tracing::debug!(kind = kind.slug(), "insight dropped: no valid evidence");
                return None;
            }

            // A claim that the player has got better is a claim about two
            // points in time, and only `progress.*` evidence describes two.
            // The prompt says so; this is what makes it true. Without it a
            // model can call any good current figure an improvement, cite the
            // current figure quite correctly, and produce a sentence the
            // player has no way to check.
            if kind == InsightKind::Improvement && !refs.iter().any(|id| is_progress(id)) {
                tracing::warn!(
                    "insight dropped: claimed an improvement without comparing two sessions"
                );
                return None;
            }

            let title = clamp(raw.title.trim(), 80);
            let explanation = clamp(raw.explanation.trim(), 600);

            // The three-part form. Each part is shorter than a lone paragraph
            // would be, because three short answers are the point of splitting
            // it — a 600-character "what happened" is a paragraph again.
            let what_happened = optional(&raw.what_happened, 400);
            let why_it_matters = optional(&raw.why_it_matters, 400);
            let better_action = optional(&raw.better_action, 400);

            // One shape or the other, and the split form is only a split form
            // when all three parts are there: two thirds of it is a paragraph
            // with a heading missing, and the missing third is always the one
            // the player came for.
            let structured =
                what_happened.is_some() && why_it_matters.is_some() && better_action.is_some();
            if title.is_empty() || (explanation.is_empty() && !structured) {
                return None;
            }

            // A citation says where a claim came from; this says the claim is
            // true to it. Both are needed — a real id attached to an invented
            // figure reads exactly like a verified one.
            let prose: Vec<&str> = [
                Some(title.as_str()),
                Some(explanation.as_str()),
                what_happened.as_deref(),
                why_it_matters.as_deref(),
                better_action.as_deref(),
            ]
            .into_iter()
            .flatten()
            .collect();

            if let Some(invented) = invented_figure(&prose, &refs, evidence) {
                tracing::warn!(
                    kind = kind.slug(),
                    figure = invented,
                    "insight dropped: stated a figure the evidence does not contain",
                );
                return None;
            }

            // And the same rule for timestamps, checked as strings. A moment the
            // evidence never recorded is a fabricated reading of a replay, which
            // is worse than a wrong number: a player cannot tell it from a real
            // one without opening the game.
            if let Some(invented) = invented_clock(&prose, &refs, evidence) {
                tracing::warn!(
                    kind = kind.slug(),
                    timestamp = invented,
                    "insight dropped: named a moment the evidence does not contain",
                );
                return None;
            }

            // Stripped rather than fatal: the prose above is already verified, so
            // an unsupported heading loses the heading and keeps the coaching.
            let timestamp = optional(&raw.timestamp, 12).filter(|stated| {
                let grounded = cited_statements(&refs, evidence)
                    .iter()
                    .any(|source| source.contains(stated.as_str()));
                if !grounded {
                    tracing::warn!(
                        timestamp = %stated,
                        "insight timestamp dropped: not present in the cited evidence"
                    );
                }
                grounded
            });

            Some(Insight {
                kind,
                kind_label: kind.label(),
                title,
                explanation,
                severity: InsightSeverity::parse(&raw.severity),
                timestamp,
                what_happened,
                why_it_matters,
                better_action,
                evidence: refs,
            })
        })
        .take(max_insights)
        .collect();

    if insights.is_empty() {
        return Err(CoachingError::Unusable(
            "no insight cited evidence that exists".into(),
        ));
    }

    let plan: Vec<PlanStep> = parsed
        .plan
        .into_iter()
        .filter_map(|raw| {
            let refs = cited(raw.evidence, &known);
            if refs.is_empty() {
                tracing::debug!("plan step dropped: no valid evidence");
                return None;
            }

            let title = clamp(raw.title.trim(), 80);
            let action = clamp(raw.action.trim(), 400);
            if title.is_empty() || action.is_empty() {
                return None;
            }

            if let Some(invented) = invented_figure(&[&title, &action], &refs, evidence) {
                tracing::warn!(
                    figure = invented,
                    "plan step dropped: stated a figure the evidence does not contain",
                );
                return None;
            }

            if let Some(invented) = invented_clock(&[&title, &action], &refs, evidence) {
                tracing::warn!(
                    timestamp = invented,
                    "plan step dropped: named a moment the evidence does not contain",
                );
                return None;
            }

            Some(PlanStep {
                // Renumbered after the drops, so the plan a player reads is
                // always 1..n with no holes where a rejected step used to be.
                position: 0,
                title,
                action,
                evidence: refs,
            })
        })
        .take(max_plan_steps)
        .enumerate()
        .map(|(index, step)| PlanStep {
            position: index as u32 + 1,
            ..step
        })
        .collect();

    // The summary is a synthesis of everything it was shown rather than a claim
    // about one item, so it is checked against the whole evidence set.
    let all: Vec<&str> = evidence.iter().map(|e| e.statement.as_str()).collect();
    let summary = clamp(parsed.summary.trim(), 400);
    let summary = match numbers::unverifiable(&summary, &all).first() {
        Some(figure) => {
            tracing::warn!(
                figure,
                "summary dropped: stated a figure the evidence does not contain"
            );
            String::new()
        }
        None => summary,
    };

    Ok(Draft {
        summary,
        insights,
        plan,
    })
}

/// Keep only the citations that name evidence which actually exists.
/// Whether an evidence id describes change over time rather than a current
/// reading.
///
/// `progress.none` counts: it is the statement that there is nothing to
/// compare, and an "improvement" insight citing only that is claiming an
/// improvement from an explicit absence of one — which the figure-grounding
/// check below will not catch, because there is no figure in it.
fn is_progress(id: &str) -> bool {
    id.starts_with("progress.") && id != "progress.none"
}

fn cited(refs: EvidenceRefs, known: &HashSet<&str>) -> Vec<String> {
    refs.into_vec()
        .into_iter()
        .filter(|id| known.contains(id.as_str()))
        .collect()
}

/// The statements behind a set of citations.
fn cited_statements<'a>(refs: &[String], evidence: &'a [Evidence]) -> Vec<&'a str> {
    evidence
        .iter()
        .filter(|e| refs.iter().any(|id| id == &e.id))
        .map(|e| e.statement.as_str())
        .collect()
}

/// The first figure in `texts` that the cited evidence does not contain.
fn invented_figure(texts: &[&str], refs: &[String], evidence: &[Evidence]) -> Option<f64> {
    let sources = cited_statements(refs, evidence);

    texts
        .iter()
        .find_map(|text| numbers::unverifiable(text, &sources).first().copied())
}

/// The first `m:ss` in `texts` that the cited evidence does not contain.
fn invented_clock(texts: &[&str], refs: &[String], evidence: &[Evidence]) -> Option<String> {
    let sources = cited_statements(refs, evidence);

    texts.iter().find_map(|text| {
        numbers::unverifiable_clocks(text, &sources)
            .into_iter()
            .next()
    })
}

/// A trimmed, clamped field, or `None` when the model left it blank.
///
/// `None` rather than an empty string so the absence survives serialization:
/// a stored `""` and a stored `null` render the same in a template and mean
/// different things to a reader of the JSON.
fn optional(value: &str, max_chars: usize) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| clamp(trimmed, max_chars))
}

/// Find the JSON object in a model answer.
///
/// Providers that ignore `response_format` wrap the object in prose or a
/// fenced code block. Taking the outermost braces is enough for both, and a
/// genuinely malformed answer still fails at parse time.
fn extract_json(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    (end > start).then(|| &raw[start..=end])
}

/// Truncate on a character boundary, without cutting a word in half when it
/// can be avoided.
fn clamp(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let truncated: String = value.chars().take(max_chars).collect();
    match truncated.rsplit_once(' ') {
        Some((head, _)) if head.chars().count() > max_chars / 2 => format!("{head}…"),
        _ => format!("{truncated}…"),
    }
}

/// Identity of one analysis request.
///
/// Two requests with the same evidence, model configuration and prompt version
/// have the same answer, so the hash is both a cache key and a guard against
/// spending a user's daily budget on a question already answered.
pub fn context_hash(
    evidence: &[Evidence],
    scope: AnalysisScope,
    role: Option<crate::domain::role::CoachableRole>,
    model: &str,
) -> String {
    let mut hasher = Sha256::new();

    hasher.update(prompt::PROMPT_VERSION.to_le_bytes());
    hasher.update(format!("{scope:?}").as_bytes());
    // Explicit rather than incidental. The role does appear inside the scope
    // evidence statement today, but a cache key that depended on the wording of
    // a sentence would break the moment somebody edited the sentence.
    hasher.update(role.map(|r| r.slug()).unwrap_or("none").as_bytes());
    hasher.update(model.as_bytes());
    for item in evidence {
        hasher.update(item.id.as_bytes());
        hasher.update(b"\x1f");
        hasher.update(item.statement.as_bytes());
        hasher.update(b"\x1e");
    }

    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::benchmark::Confidence;
    use crate::domain::coaching::EvidenceKind;
    use crate::domain::role::CoachableRole;

    fn evidence() -> Vec<Evidence> {
        [
            (
                "overall.record",
                "Across 20 stored matches, you have won 11 and lost 9 (55%).",
            ),
            (
                "benchmark.gold_per_min",
                "On Luna, your gold per minute averages 511.8; the peer median is 558.0.",
            ),
        ]
        .into_iter()
        .map(|(id, statement)| Evidence {
            id: id.to_string(),
            kind: EvidenceKind::Overall,
            label: "Label".into(),
            statement: statement.to_string(),
            sample: 20,
            confidence: Confidence::Adequate,
        })
        .collect()
    }

    /// The same, plus a progress item and the explicit "nothing to compare"
    /// marker, for the improvement rule.
    fn evidence_with_progress() -> Vec<Evidence> {
        let mut out = evidence();
        for (id, statement) in [
            (
                "progress.overall.deaths",
                "Deaths per 10 minutes improved to 5.80, from 7.20 at your previous coaching session.",
            ),
            (
                "progress.none",
                "This is the first coaching session recorded for this role.",
            ),
        ] {
            out.push(Evidence {
                id: id.to_string(),
                kind: EvidenceKind::Progress,
                label: "Progress".into(),
                statement: statement.to_string(),
                sample: 20,
                confidence: Confidence::Adequate,
            });
        }
        out
    }

    fn answer(insights: &str) -> String {
        format!(r#"{{"summary": "You are farming well.", "insights": [{insights}]}}"#)
    }

    /// An answer with a plan attached, for the steps' own rules.
    #[test]
    fn an_improvement_claim_without_a_comparison_is_dropped() {
        // The model cites a real id and writes a true-looking sentence. The
        // citation is current-state evidence, so the claim that anything got
        // better rests on nothing.
        let raw = answer(
            r#"{"kind":"improvement","title":"Your farm is better",
                "explanation":"You are farming more than you were.",
                "evidence":["benchmark.gold_per_min"]}"#,
        );

        let result = parse_analysis(&raw, &evidence(), 5, 4);
        assert!(
            matches!(result, Err(CoachingError::Unusable(_))),
            "an unverifiable improvement should not survive as the only insight",
        );
    }

    #[test]
    fn an_improvement_citing_a_comparison_survives() {
        let raw = answer(
            r#"{"kind":"improvement","title":"You are dying less",
                "explanation":"Deaths came down to 5.80 from 7.20 since last time.",
                "evidence":["progress.overall.deaths"]}"#,
        );

        let draft = parse_analysis(&raw, &evidence_with_progress(), 5, 4).unwrap();
        assert_eq!(draft.insights.len(), 1);
        assert_eq!(draft.insights[0].kind, InsightKind::Improvement);
    }

    #[test]
    fn progress_none_does_not_license_an_improvement_claim() {
        // "There is nothing to compare against" is the one progress item that
        // must not support a claim of change. It contains no figure, so the
        // number check cannot catch this one.
        let raw = answer(
            r#"{"kind":"improvement","title":"You have improved",
                "explanation":"Things are looking up.",
                "evidence":["progress.none"]}"#,
        );

        let result = parse_analysis(&raw, &evidence_with_progress(), 5, 4);
        assert!(matches!(result, Err(CoachingError::Unusable(_))));
    }

    #[test]
    fn other_insight_kinds_are_unaffected_by_the_progress_rule() {
        let raw = answer(
            r#"{"kind":"weakness","title":"Farm is behind the median",
                "explanation":"Your gold per minute sits below your peers.",
                "evidence":["benchmark.gold_per_min"]}"#,
        );

        let draft = parse_analysis(&raw, &evidence(), 5, 4).unwrap();
        assert_eq!(draft.insights.len(), 1);
    }

    fn answer_with_plan(insights: &str, plan: &str) -> String {
        format!(
            r#"{{"summary": "You are farming well.", "insights": [{insights}], "plan": [{plan}]}}"#
        )
    }

    const GOOD: &str = r#"{
        "kind": "weakness",
        "title": "Your farm trails your rank",
        "explanation": "The gap is small but consistent.",
        "evidence": ["benchmark.gold_per_min"]
    }"#;

    #[test]
    fn a_well_formed_answer_is_accepted() {
        let draft = parse_analysis(&answer(GOOD), &evidence(), 5, 4).unwrap();

        assert_eq!(draft.summary, "You are farming well.");
        assert_eq!(draft.insights.len(), 1);
        assert_eq!(draft.insights[0].kind, InsightKind::Weakness);
        assert_eq!(draft.insights[0].kind_label, "Weakness");
        assert_eq!(draft.insights[0].evidence, vec!["benchmark.gold_per_min"]);
    }

    #[test]
    fn an_insight_citing_evidence_that_does_not_exist_is_dropped() {
        let invented = r#"{
            "kind": "weakness",
            "title": "Your wards are poor",
            "explanation": "You place 2.1 wards per game, well below average.",
            "evidence": ["benchmark.wards_placed"]
        }"#;

        // The invented citation is the only one, so nothing survives.
        let error = parse_analysis(&answer(invented), &evidence(), 5, 4).unwrap_err();
        assert!(matches!(error, CoachingError::Unusable(_)));
    }

    #[test]
    fn an_invented_citation_is_stripped_without_losing_a_valid_one() {
        let mixed = r#"{
            "kind": "weakness",
            "title": "Farm",
            "explanation": "Detail.",
            "evidence": ["benchmark.gold_per_min", "benchmark.wards_placed"]
        }"#;

        let draft = parse_analysis(&answer(mixed), &evidence(), 5, 4).unwrap();
        assert_eq!(draft.insights[0].evidence, vec!["benchmark.gold_per_min"]);
    }

    #[test]
    fn an_invented_kind_is_discarded_rather_than_coerced() {
        let invented_kind = format!(
            r#"{{"kind": "observation", "title": "T", "explanation": "E", "evidence": ["overall.record"]}}, {GOOD}"#
        );

        let draft = parse_analysis(&answer(&invented_kind), &evidence(), 5, 4).unwrap();
        assert_eq!(draft.insights.len(), 1);
        assert_eq!(draft.insights[0].kind, InsightKind::Weakness);
    }

    #[test]
    fn a_single_citation_sent_as_a_bare_string_is_accepted() {
        let bare = r#"{
            "kind": "strength",
            "title": "Good economy",
            "explanation": "Detail.",
            "evidence": "overall.record"
        }"#;

        let draft = parse_analysis(&answer(bare), &evidence(), 5, 4).unwrap();
        assert_eq!(draft.insights[0].evidence, vec!["overall.record"]);
    }

    #[test]
    fn json_wrapped_in_prose_or_a_code_fence_is_still_read() {
        let fenced = format!(
            "Sure! Here is the analysis:\n```json\n{}\n```",
            answer(GOOD)
        );
        let draft = parse_analysis(&fenced, &evidence(), 5, 4).unwrap();
        assert_eq!(draft.insights.len(), 1);
    }

    #[test]
    fn an_answer_with_no_json_is_unusable() {
        let error = parse_analysis("I cannot help with that.", &evidence(), 5, 4).unwrap_err();
        assert!(matches!(error, CoachingError::Unusable(_)));
    }

    #[test]
    fn malformed_json_is_unusable_rather_than_a_panic() {
        let error = parse_analysis("{\"summary\": ", &evidence(), 5, 4).unwrap_err();
        assert!(matches!(error, CoachingError::Unusable(_)));
    }

    #[test]
    fn the_insight_cap_is_enforced_on_the_answer() {
        let many = std::iter::repeat_n(GOOD, 9).collect::<Vec<_>>().join(",");
        let draft = parse_analysis(&answer(&many), &evidence(), 3, 4).unwrap();

        assert_eq!(draft.insights.len(), 3);
    }

    #[test]
    fn overlong_text_is_truncated_on_a_character_boundary() {
        let long = "é".repeat(900);
        let raw = format!(
            r#"{{"summary": "{long}", "insights": [{{"kind": "strength", "title": "T", "explanation": "{long}", "evidence": ["overall.record"]}}]}}"#
        );

        let draft = parse_analysis(&raw, &evidence(), 5, 4).unwrap();
        assert!(draft.summary.chars().count() <= 401);
        assert!(draft.insights[0].explanation.chars().count() <= 601);
    }

    #[test]
    fn an_insight_with_no_text_is_dropped() {
        let empty = r#"{"kind": "strength", "title": "", "explanation": "", "evidence": ["overall.record"]}"#;
        assert!(parse_analysis(&answer(empty), &evidence(), 5, 4).is_err());
    }

    #[test]
    fn a_training_plan_is_parsed_and_numbered_from_one() {
        let plan = r#"{
            "title": "Leave fights you have not set up",
            "action": "Only commit when you know where the enemy support is.",
            "evidence": ["overall.record"]
        }, {
            "title": "Check buyback before committing",
            "action": "Look at your gold before every fight.",
            "evidence": ["benchmark.gold_per_min"]
        }"#;

        let draft = parse_analysis(&answer_with_plan(GOOD, plan), &evidence(), 5, 4).unwrap();

        assert_eq!(draft.plan.len(), 2);
        assert_eq!(draft.plan[0].position, 1);
        assert_eq!(draft.plan[1].position, 2);
        assert_eq!(draft.plan[0].evidence, vec!["overall.record"]);
    }

    #[test]
    fn a_plan_step_citing_nothing_real_is_dropped_and_the_rest_renumbered() {
        let plan = r#"{
            "title": "Invented",
            "action": "Based on data nobody has.",
            "evidence": ["benchmark.wards_placed"]
        }, {
            "title": "Real",
            "action": "Based on something measured.",
            "evidence": ["overall.record"]
        }"#;

        let draft = parse_analysis(&answer_with_plan(GOOD, plan), &evidence(), 5, 4).unwrap();

        // The survivor is step one, not step two with a hole in front of it.
        assert_eq!(draft.plan.len(), 1);
        assert_eq!(draft.plan[0].title, "Real");
        assert_eq!(draft.plan[0].position, 1);
    }

    #[test]
    fn the_plan_cap_is_enforced() {
        let step = r#"{"title": "T", "action": "A", "evidence": ["overall.record"]}"#;
        let many = std::iter::repeat_n(step, 9).collect::<Vec<_>>().join(",");

        let draft = parse_analysis(&answer_with_plan(GOOD, &many), &evidence(), 5, 3).unwrap();
        assert_eq!(draft.plan.len(), 3);
    }

    #[test]
    fn an_answer_with_no_plan_is_still_a_usable_analysis() {
        // Not every analysis warrants a plan, and an empty one is an answer.
        let draft = parse_analysis(&answer(GOOD), &evidence(), 5, 4).unwrap();
        assert!(draft.plan.is_empty());
        assert_eq!(draft.insights.len(), 1);
    }

    /// The rule citation validation cannot enforce on its own: a real id
    /// attached to an invented figure reads exactly like a verified one.
    #[test]
    fn an_insight_stating_a_figure_the_evidence_does_not_contain_is_dropped() {
        let invented_figure = r#"{
            "kind": "weakness",
            "title": "Your farm trails",
            "explanation": "You average 412 gold per minute, well short of the mark.",
            "evidence": ["benchmark.gold_per_min"]
        }"#;

        // The citation is real. The number is not in it, so nothing survives.
        let error = parse_analysis(&answer(invented_figure), &evidence(), 5, 4).unwrap_err();
        assert!(matches!(error, CoachingError::Unusable(_)));
    }

    #[test]
    fn an_insight_repeating_a_measured_figure_is_kept() {
        let faithful = r#"{
            "kind": "weakness",
            "title": "Twenty matches is a thin read",
            "explanation": "Across 20 matches the trend is real but young.",
            "evidence": ["overall.record"]
        }"#;

        let draft = parse_analysis(&answer(faithful), &evidence(), 5, 4).unwrap();
        assert_eq!(draft.insights.len(), 1);
    }

    #[test]
    fn a_plan_step_that_invents_a_timing_target_is_dropped() {
        // The failure this rule exists for: advice that sounds authoritative
        // and rests on a number nobody measured.
        let plan = r#"{
            "title": "Hit your item timing",
            "action": "Finish your first big item before 18 minutes.",
            "evidence": ["overall.record"]
        }"#;

        let draft = parse_analysis(&answer_with_plan(GOOD, plan), &evidence(), 5, 4).unwrap();
        assert!(draft.plan.is_empty());
        // And the insight beside it is untouched: one bad step is not a bad
        // answer.
        assert_eq!(draft.insights.len(), 1);
    }

    #[test]
    fn a_summary_that_invents_a_figure_is_dropped_without_losing_the_insights() {
        let raw = r#"{
            "summary": "You are winning 73% of your games right now.",
            "insights": [GOOD_INSIGHT]
        }"#
        .replace("GOOD_INSIGHT", GOOD);

        let draft = parse_analysis(&raw, &evidence(), 5, 4).unwrap();

        assert!(
            draft.summary.is_empty(),
            "an invented headline figure must not reach a user: {}",
            draft.summary,
        );
        assert_eq!(draft.insights.len(), 1, "the verified work survives");
    }

    // -----------------------------------------------------------------------
    // Single-match analysis: the three-part form, severity, and timestamps
    // -----------------------------------------------------------------------

    /// Evidence with a timeline in it, as a single-match analysis is given.
    fn timeline_evidence() -> Vec<Evidence> {
        [
            (
                "match.timeline.deaths",
                "You died 3 times, at 10:12 to Lion, 18:42 to Axe, and 27:04 to Lion.",
            ),
            (
                "match.timeline.items",
                "You completed Blink Dagger at 14:22, Black King Bar at 21:44.",
            ),
        ]
        .into_iter()
        .map(|(id, statement)| Evidence {
            id: id.to_string(),
            kind: EvidenceKind::Match,
            label: "Timeline".into(),
            statement: statement.to_string(),
            sample: 1,
            confidence: Confidence::Insufficient,
        })
        .collect()
    }

    const STRUCTURED: &str = r#"{
        "kind": "weakness",
        "title": "You took the fight alone",
        "severity": "major",
        "timestamp": "18:42",
        "what_happened": "You engaged without your team nearby.",
        "why_it_matters": "They were grouped and you were the only target, so the trade was free for them.",
        "better_action": "Wait for your team to move together before you commit.",
        "evidence": ["match.timeline.deaths"]
    }"#;

    #[test]
    fn a_match_insight_keeps_its_three_parts_severity_and_timestamp() {
        let draft = parse_analysis(&answer(STRUCTURED), &timeline_evidence(), 5, 1).unwrap();
        let insight = &draft.insights[0];

        assert_eq!(insight.severity, Some(InsightSeverity::Major));
        assert_eq!(insight.timestamp.as_deref(), Some("18:42"));
        assert_eq!(
            insight.what_happened.as_deref(),
            Some("You engaged without your team nearby.")
        );
        assert!(insight.why_it_matters.is_some());
        assert!(insight.better_action.is_some());
        // The three-part form replaces the paragraph rather than duplicating it.
        assert!(insight.explanation.is_empty());
    }

    /// The failure the string match exists for. Both halves of `18:45` appear in
    /// the cited statement as bare numbers, so the figure checker is satisfied by
    /// a moment that never happened.
    #[test]
    fn a_match_insight_naming_a_moment_the_evidence_never_recorded_is_dropped() {
        // 18 and 45 both appear in the cited statement as bare numbers (18 from
        // "18:42", 45 from nothing in particular), so the figure checker passes
        // this and only the string match catches it.
        let fabricated = STRUCTURED.replace(
            "You engaged without your team nearby.",
            "At 18:45 you engaged without your team nearby.",
        );

        let error = parse_analysis(&answer(&fabricated), &timeline_evidence(), 5, 1).unwrap_err();
        assert!(matches!(error, CoachingError::Unusable(_)));
    }

    /// A timestamp is a heading, not the coaching. Losing it must not lose the
    /// three verified paragraphs under it.
    #[test]
    fn an_unsupported_timestamp_field_is_stripped_without_dropping_the_insight() {
        // "9:03" is clock-shaped and absent from the evidence, but it appears
        // only in the `timestamp` field — no prose claims it.
        let raw = STRUCTURED.replace("\"timestamp\": \"18:42\"", "\"timestamp\": \"9:03\"");

        let draft = parse_analysis(&answer(&raw), &timeline_evidence(), 5, 1).unwrap();

        assert_eq!(draft.insights.len(), 1);
        assert_eq!(draft.insights[0].timestamp, None);
        assert!(draft.insights[0].better_action.is_some());
    }

    #[test]
    fn a_timestamp_quoted_from_the_evidence_survives_in_the_prose_too() {
        let quoting = STRUCTURED.replace(
            "You engaged without your team nearby.",
            "At 18:42 you engaged without your team nearby.",
        );

        let draft = parse_analysis(&answer(&quoting), &timeline_evidence(), 5, 1).unwrap();
        assert!(draft.insights[0]
            .what_happened
            .as_deref()
            .unwrap()
            .contains("18:42"));
    }

    /// Two thirds of the three-part form is a paragraph with a heading missing,
    /// and the missing third is always the advice the player came for.
    #[test]
    fn a_partial_three_part_insight_is_not_accepted_as_one() {
        let no_advice = r#"{
            "kind": "weakness",
            "title": "You took the fight alone",
            "severity": "major",
            "what_happened": "You engaged without your team nearby.",
            "why_it_matters": "The trade was free for them.",
            "evidence": ["match.timeline.deaths"]
        }"#;

        let error = parse_analysis(&answer(no_advice), &timeline_evidence(), 5, 1).unwrap_err();
        assert!(matches!(error, CoachingError::Unusable(_)));
    }

    /// The other shape still works, unchanged: a stored analysis and a
    /// player-wide answer both use `explanation`.
    #[test]
    fn the_paragraph_form_is_unaffected_and_leaves_the_new_fields_empty() {
        let draft = parse_analysis(&answer(GOOD), &evidence(), 5, 4).unwrap();
        let insight = &draft.insights[0];

        assert!(!insight.explanation.is_empty());
        assert_eq!(insight.severity, None);
        assert_eq!(insight.what_happened, None);
        assert_eq!(insight.timestamp, None);
    }

    /// Absent is not "minor". A severity the model did not supply must not be
    /// invented on its behalf, in either direction.
    #[test]
    fn an_unrecognised_severity_is_dropped_rather_than_defaulted() {
        for value in ["critical", "MODERATE", ""] {
            let raw = STRUCTURED.replace(
                "\"severity\": \"major\"",
                &format!("\"severity\": \"{value}\""),
            );
            let draft = parse_analysis(&answer(&raw), &timeline_evidence(), 5, 1).unwrap();

            assert_eq!(
                draft.insights[0].severity, None,
                "\"{value}\" should not become a severity"
            );
        }

        // And the two real ones are read, case- and padding-insensitively.
        let minor = STRUCTURED.replace("\"major\"", "\" Minor \"");
        let draft = parse_analysis(&answer(&minor), &timeline_evidence(), 5, 1).unwrap();
        assert_eq!(draft.insights[0].severity, Some(InsightSeverity::Minor));
    }

    #[test]
    fn camel_cased_keys_are_accepted_because_models_reach_for_them() {
        let camel = r#"{
            "kind": "weakness",
            "title": "You took the fight alone",
            "severity": "major",
            "whatHappened": "You engaged without your team nearby.",
            "whyItMatters": "The trade was free for them.",
            "betterAction": "Move with your team.",
            "evidence": ["match.timeline.deaths"]
        }"#;

        let draft = parse_analysis(&answer(camel), &timeline_evidence(), 5, 1).unwrap();
        assert!(draft.insights[0].better_action.is_some());
    }

    /// A plan step that invents a moment is dropped for the same reason an
    /// insight is: a fabricated replay reading is unfalsifiable to the reader.
    #[test]
    fn a_plan_step_naming_an_invented_moment_is_dropped() {
        let plan = r#"{
            "title": "Fight selection",
            "action": "Do not repeat the 31:15 mistake.",
            "evidence": ["match.timeline.deaths"]
        }"#;

        let draft = parse_analysis(
            &answer_with_plan(STRUCTURED, plan),
            &timeline_evidence(),
            5,
            1,
        )
        .unwrap();

        assert!(draft.plan.is_empty());
        assert_eq!(draft.insights.len(), 1, "the verified insight survives");
    }

    /// The single primary training focus, enforced as a number rather than asked
    /// for in the prompt: a player handed four things to change from one game
    /// changes none of them.
    #[test]
    fn a_match_analysis_produces_exactly_one_plan_step() {
        let step = r#"{"title": "T", "action": "A", "evidence": ["match.timeline.deaths"]}"#;
        let many = std::iter::repeat_n(step, 4).collect::<Vec<_>>().join(",");

        let draft = parse_analysis(
            &answer_with_plan(STRUCTURED, &many),
            &timeline_evidence(),
            5,
            1,
        )
        .unwrap();

        assert_eq!(draft.plan.len(), 1);
        assert_eq!(draft.plan[0].position, 1);
    }

    #[test]
    fn the_plan_cap_is_one_for_a_match_and_configured_for_everything_else() {
        assert_eq!(plan_cap(AnalysisScope::Match, 4), 1);
        assert_eq!(plan_cap(AnalysisScope::Role, 4), 4);
        assert_eq!(plan_cap(AnalysisScope::Player, 4), 4);
    }

    #[test]
    fn the_context_hash_changes_with_the_evidence_and_not_otherwise() {
        let base = context_hash(&evidence(), AnalysisScope::Player, None, "m");

        assert_eq!(
            base,
            context_hash(&evidence(), AnalysisScope::Player, None, "m")
        );
        assert_ne!(
            base,
            context_hash(&evidence(), AnalysisScope::Match, None, "m")
        );
        assert_ne!(
            base,
            context_hash(&evidence(), AnalysisScope::Player, None, "m2")
        );

        let mut changed = evidence();
        changed[0].statement = "A different measured sentence.".into();
        assert_ne!(
            base,
            context_hash(&changed, AnalysisScope::Player, None, "m")
        );
    }

    /// The guard against the worst silent failure in role coaching: two roles
    /// sharing a cache entry, so a player is handed an analysis of a role they
    /// did not ask about with nothing to indicate it.
    #[test]
    fn the_context_hash_separates_two_roles_with_identical_evidence() {
        let carry = context_hash(
            &evidence(),
            AnalysisScope::Role,
            Some(CoachableRole::Carry),
            "m",
        );
        let support = context_hash(
            &evidence(),
            AnalysisScope::Role,
            Some(CoachableRole::SoftSupport),
            "m",
        );
        let unscoped = context_hash(&evidence(), AnalysisScope::Role, None, "m");

        assert_ne!(carry, support);
        assert_ne!(carry, unscoped);
        assert_ne!(support, unscoped);
    }

    #[test]
    fn the_context_hash_is_order_sensitive_because_the_prompt_is() {
        let mut reordered = evidence();
        reordered.reverse();

        assert_ne!(
            context_hash(&evidence(), AnalysisScope::Player, None, "m"),
            context_hash(&reordered, AnalysisScope::Player, None, "m")
        );
    }
}
