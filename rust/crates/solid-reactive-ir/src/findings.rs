//! The dialect-neutral diagnostic model.
//!
//! Every dialect's rule catalog produces these findings from a [`Program`],
//! and the backend consumes them without knowing which catalog ran. The
//! catalogs themselves — which rules exist, their codes, messages, and when
//! they fire — live in the dialect crates.
//!
//! [`Program`]: crate::Program

use serde::{Deserialize, Serialize};
use std::time::Duration;
use typefacts::Location;

use crate::Fix;

/// Where a catalog run spent its time.
#[derive(Clone, Copy, Debug, Default)]
pub struct SolveTimings {
    pub total: Duration,
    pub finding_construction: Duration,
    pub final_ordering: Duration,
}

/// The externally visible identity of one rule: what a catalog entry
/// contributes to every finding it constructs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuleMetadata {
    pub code: &'static str,
    pub name: &'static str,
    pub severity: &'static str,
    pub uncertifiable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceStep {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    pub rule: String,
    pub kind: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hint: String,
    pub severity: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub analysis_context: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub subject_kind: String,
    pub primary_location: Location,
    pub related_locations: Vec<Location>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceStep>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<Fix>,
}

impl Finding {
    pub fn new(metadata: RuleMetadata, message: String, primary_location: Location) -> Self {
        Self {
            id: metadata.code.into(),
            rule: metadata.name.into(),
            kind: if metadata.uncertifiable {
                "uncertifiable".into()
            } else {
                "violation".into()
            },
            message,
            hint: String::new(),
            severity: metadata.severity.into(),
            analysis_context: String::new(),
            subject_kind: String::new(),
            primary_location,
            related_locations: vec![],
            evidence: vec![],
            fixes: vec![],
        }
    }
}
