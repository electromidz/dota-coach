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

pub mod evidence;
pub mod prompt;

use std::collections::HashSet;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::config::CoachConfig;
use crate::domain::coaching::{AnalysisScope, Evidence, Insight, InsightKind};
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

    let request = LlmRequest {
        system: prompt::system(config.max_insights),
        user: prompt::user(scope, evidence),
        max_output_tokens: config.max_output_tokens,
        temperature: config.temperature,
        json_only: true,
    };

    let completion = llm.generate(&request).await?;
    let draft = parse_analysis(&completion.text, evidence, config.max_insights)?;

    Ok(Generated {
        summary: draft.summary,
        insights: draft.insights,
        model: completion.model,
    })
}

#[derive(Debug)]
pub struct Draft {
    pub summary: String,
    pub insights: Vec<Insight>,
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
}

#[derive(Debug, Deserialize)]
struct RawInsight {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    explanation: String,
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

            let refs: Vec<String> = raw
                .evidence
                .into_vec()
                .into_iter()
                .filter(|id| known.contains(id.as_str()))
                .collect();

            // The rule the whole phase rests on: no evidence, no insight.
            if refs.is_empty() {
                tracing::debug!(kind = kind.slug(), "insight dropped: no valid evidence");
                return None;
            }

            let title = clamp(raw.title.trim(), 80);
            let explanation = clamp(raw.explanation.trim(), 600);
            if title.is_empty() || explanation.is_empty() {
                return None;
            }

            Some(Insight {
                kind,
                kind_label: kind.label(),
                title,
                explanation,
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

    Ok(Draft {
        summary: clamp(parsed.summary.trim(), 400),
        insights,
    })
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
pub fn context_hash(evidence: &[Evidence], scope: AnalysisScope, model: &str) -> String {
    let mut hasher = Sha256::new();

    hasher.update(prompt::PROMPT_VERSION.to_le_bytes());
    hasher.update(format!("{scope:?}").as_bytes());
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

    fn evidence() -> Vec<Evidence> {
        ["overall.record", "benchmark.gold_per_min"]
            .into_iter()
            .map(|id| Evidence {
                id: id.to_string(),
                kind: EvidenceKind::Overall,
                label: "Label".into(),
                statement: "A measured sentence.".into(),
                sample: 20,
                confidence: Confidence::Adequate,
            })
            .collect()
    }

    fn answer(insights: &str) -> String {
        format!(r#"{{"summary": "You are farming well.", "insights": [{insights}]}}"#)
    }

    const GOOD: &str = r#"{
        "kind": "weakness",
        "title": "Your farm trails your rank",
        "explanation": "The gap is small but consistent.",
        "evidence": ["benchmark.gold_per_min"]
    }"#;

    #[test]
    fn a_well_formed_answer_is_accepted() {
        let draft = parse_analysis(&answer(GOOD), &evidence(), 5).unwrap();

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
        let error = parse_analysis(&answer(invented), &evidence(), 5).unwrap_err();
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

        let draft = parse_analysis(&answer(mixed), &evidence(), 5).unwrap();
        assert_eq!(draft.insights[0].evidence, vec!["benchmark.gold_per_min"]);
    }

    #[test]
    fn an_invented_kind_is_discarded_rather_than_coerced() {
        let invented_kind = format!(
            r#"{{"kind": "observation", "title": "T", "explanation": "E", "evidence": ["overall.record"]}}, {GOOD}"#
        );

        let draft = parse_analysis(&answer(&invented_kind), &evidence(), 5).unwrap();
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

        let draft = parse_analysis(&answer(bare), &evidence(), 5).unwrap();
        assert_eq!(draft.insights[0].evidence, vec!["overall.record"]);
    }

    #[test]
    fn json_wrapped_in_prose_or_a_code_fence_is_still_read() {
        let fenced = format!(
            "Sure! Here is the analysis:\n```json\n{}\n```",
            answer(GOOD)
        );
        let draft = parse_analysis(&fenced, &evidence(), 5).unwrap();
        assert_eq!(draft.insights.len(), 1);
    }

    #[test]
    fn an_answer_with_no_json_is_unusable() {
        let error = parse_analysis("I cannot help with that.", &evidence(), 5).unwrap_err();
        assert!(matches!(error, CoachingError::Unusable(_)));
    }

    #[test]
    fn malformed_json_is_unusable_rather_than_a_panic() {
        let error = parse_analysis("{\"summary\": ", &evidence(), 5).unwrap_err();
        assert!(matches!(error, CoachingError::Unusable(_)));
    }

    #[test]
    fn the_insight_cap_is_enforced_on_the_answer() {
        let many = std::iter::repeat_n(GOOD, 9).collect::<Vec<_>>().join(",");
        let draft = parse_analysis(&answer(&many), &evidence(), 3).unwrap();

        assert_eq!(draft.insights.len(), 3);
    }

    #[test]
    fn overlong_text_is_truncated_on_a_character_boundary() {
        let long = "é".repeat(900);
        let raw = format!(
            r#"{{"summary": "{long}", "insights": [{{"kind": "strength", "title": "T", "explanation": "{long}", "evidence": ["overall.record"]}}]}}"#
        );

        let draft = parse_analysis(&raw, &evidence(), 5).unwrap();
        assert!(draft.summary.chars().count() <= 401);
        assert!(draft.insights[0].explanation.chars().count() <= 601);
    }

    #[test]
    fn an_insight_with_no_text_is_dropped() {
        let empty = r#"{"kind": "strength", "title": "", "explanation": "", "evidence": ["overall.record"]}"#;
        assert!(parse_analysis(&answer(empty), &evidence(), 5).is_err());
    }

    #[test]
    fn the_context_hash_changes_with_the_evidence_and_not_otherwise() {
        let base = context_hash(&evidence(), AnalysisScope::Player, "m");

        assert_eq!(base, context_hash(&evidence(), AnalysisScope::Player, "m"));
        assert_ne!(base, context_hash(&evidence(), AnalysisScope::Match, "m"));
        assert_ne!(base, context_hash(&evidence(), AnalysisScope::Player, "m2"));

        let mut changed = evidence();
        changed[0].statement = "A different measured sentence.".into();
        assert_ne!(base, context_hash(&changed, AnalysisScope::Player, "m"));
    }

    #[test]
    fn the_context_hash_is_order_sensitive_because_the_prompt_is() {
        let mut reordered = evidence();
        reordered.reverse();

        assert_ne!(
            context_hash(&evidence(), AnalysisScope::Player, "m"),
            context_hash(&reordered, AnalysisScope::Player, "m")
        );
    }
}
