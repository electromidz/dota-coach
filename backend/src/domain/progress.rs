//! Progress: what changed between two coaching sessions.
//!
//! Every judgement here is the backend's. The model is told *that* deaths
//! improved and by how much; it is never handed two numbers and asked to work
//! it out, because "improved" is a claim about a player and a model that
//! computes it can compute it wrong in a way nothing downstream can catch.
//!
//! Two rules shape the types:
//!
//!   - **Only compatible readings are compared.** Two metrics pair when they
//!     share a key *and* a unit *and* a direction. A metric that was redefined
//!     between sessions is reported as uncomparable rather than subtracted.
//!   - **Absent is not zero, and unknown is not stable.** A metric with too
//!     thin a sample, or present in only one session, gets
//!     [`ProgressStatus::InsufficientData`] — never a quiet "no change".

use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::coaching_session::MetricUnit;
use crate::domain::role::CoachableRole;

/// What happened to one metric between two sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProgressStatus {
    /// Moved in the good direction by more than the stable band.
    Improved,
    /// Moved in the bad direction by more than the stable band.
    Declined,
    /// Moved by less than the stable band. A real answer, not a missing one.
    Stable,
    /// A recurring pattern that was not firing before and is now.
    NewIssue,
    /// A recurring pattern that was firing before and is not now.
    ResolvedIssue,
    /// Not comparable: too thin a sample, present in only one session, or
    /// measured differently in each.
    InsufficientData,
}

impl ProgressStatus {
    pub fn slug(self) -> &'static str {
        match self {
            ProgressStatus::Improved => "improved",
            ProgressStatus::Declined => "declined",
            ProgressStatus::Stable => "stable",
            ProgressStatus::NewIssue => "new_issue",
            ProgressStatus::ResolvedIssue => "resolved_issue",
            ProgressStatus::InsufficientData => "insufficient_data",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ProgressStatus::Improved => "Improved",
            ProgressStatus::Declined => "Declined",
            ProgressStatus::Stable => "Stable",
            ProgressStatus::NewIssue => "New issue",
            ProgressStatus::ResolvedIssue => "Resolved",
            ProgressStatus::InsufficientData => "Not enough data",
        }
    }

    /// Whether this is a movement claim at all. `false` for the three statuses
    /// that describe availability or existence rather than direction.
    pub fn is_movement(self) -> bool {
        matches!(
            self,
            ProgressStatus::Improved | ProgressStatus::Declined | ProgressStatus::Stable
        )
    }
}

/// One metric, then and now.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MetricProgress {
    pub key: String,
    pub label: String,
    pub unit: MetricUnit,
    pub higher_is_better: bool,

    /// The earlier reading. `None` when the metric is new.
    pub previous: Option<f32>,
    /// The later reading. `None` when the metric has stopped being measured.
    pub current: Option<f32>,

    /// `current - previous`, raw and unsigned by direction. Present only when
    /// both readings are.
    pub delta: Option<f32>,
    /// The same change, signed so **positive always means better** — including
    /// for deaths, where the raw delta runs the other way. This is the number
    /// a reader should be shown; `delta` is there for anyone reconstructing
    /// the arithmetic.
    pub direction_delta: Option<f32>,
    /// Relative change, for units that compare relatively. `None` for the
    /// bounded scales, where it would be misleading, and when the earlier
    /// reading was zero.
    pub percent_change: Option<f32>,

    pub previous_sample: Option<i64>,
    pub current_sample: Option<i64>,

    pub status: ProgressStatus,
    pub status_label: &'static str,
    /// Why, when the status is not a movement claim. Absent otherwise: a
    /// status that speaks for itself does not need a sentence.
    pub note: Option<String>,
}

/// One session pair, compared.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SessionProgress {
    pub role: CoachableRole,
    pub role_label: &'static str,

    pub previous_session_id: Uuid,
    pub previous_sequence: i32,
    pub previous_at: chrono::DateTime<chrono::Utc>,

    pub current_session_id: Uuid,
    pub current_sequence: i32,
    pub current_at: chrono::DateTime<chrono::Utc>,

    /// The headline: the role score, compared. `None` when either session
    /// could not score the role.
    pub performance: Option<MetricProgress>,
    /// Every metric that appears in either session, in the current session's
    /// order, with anything only the earlier one had appended.
    pub metrics: Vec<MetricProgress>,

    /// The movement worth leading with — the largest genuine change, or a new
    /// issue if one appeared. `None` when nothing moved.
    pub headline: Option<String>,
}

/// One metric's readings across several sessions, oldest first.
///
/// The shape behind "54 → 57 → 61". Kept separate from [`SessionProgress`]
/// because a trend and a comparison answer different questions, and a trend
/// makes no claim about whether any step was meaningful.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MetricSeries {
    pub key: String,
    pub label: String,
    pub unit: MetricUnit,
    pub higher_is_better: bool,
    pub points: Vec<SeriesPoint>,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub struct SeriesPoint {
    pub session_id: Uuid,
    pub sequence: i32,
    pub at: chrono::DateTime<chrono::Utc>,
    pub value: f32,
}
