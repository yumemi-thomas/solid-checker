//! ADR 0947: why an uncertifiable row is blocked, as typed facts in the internal JSON (`blockedBy`, and the top-level `hazards`
//! table the rows name). Uncertifiable stays internal (ADR 0328): nothing here reaches the text or ESLint outputs or an exit code,
//! and no finding kind, rule, or V/U/clean outcome moves.
//!
//! `kind` is the blocker constant the row's `analysisContext` names (`blockers.rs`), not a second enum. A veto family adds the
//! `formula` that holds the row and `fired`, the hazards present that the formula reads: a veto is a formula over whole-program
//! hazards, so no one of them "decides" it, and `scope` is `program`, the honest value today. `missing` is the typed fact that
//! would decide a row whose predicate is a single test, "the first undecided fact": the backlog counts it gives are upper bounds,
//! because proving it may expose the next hole. No emitter fills it yet.
//!
//! Locations live in one table, `hazards: [{ id, sites }]`, sorted by path then span; rows name ids and never repeat spans. A
//! hazard the census keeps no per-site record of has no sites: absent, never guessed.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use solid_facts::FileFacts;
use solid_facts::core::Span;
use typefacts::Location;

use crate::Program;
use crate::indexes::SemanticLookup;

/// The scope a veto formula's hazards have: the whole program.
pub const PROGRAM_SCOPE: &str = "program";

/// A fact whose proof would decide a row whose predicate is a single test.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Missing {
    pub fact: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<String>,
}

/// Why one uncertifiable row is blocked.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedBy {
    pub kind: String,
    /// `program` where the row's blocker is a formula over whole-program hazards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    /// The hazards present that the formula reads, by the ids of the top-level `hazards` table, sorted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fired: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing: Option<Missing>,
}

impl BlockedBy {
    /// The blocker of `analysis_context` with no more said: its head, when it is a named blocker.
    #[must_use]
    pub fn of_context(analysis_context: &str) -> Option<Self> {
        blocked_kind(analysis_context).map(|kind| Self {
            kind: kind.to_owned(),
            scope: None,
            formula: None,
            fired: Vec::new(),
            missing: None,
        })
    }
}

/// The blocker an `analysisContext` names: its head (before the first `:`), when it is a named blocker.
#[must_use]
pub fn blocked_kind(analysis_context: &str) -> Option<&str> {
    let head = analysis_context
        .split_once(':')
        .map_or(analysis_context, |(head, _)| head);
    crate::blockers::named(head).then_some(head)
}

/// A row's typed blocker, by the place of the row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedRow {
    pub location: Location,
    pub blocked_by: BlockedBy,
}

/// A site that holds a hazard.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HazardSite {
    pub path: String,
    pub start: u32,
    pub end: u32,
}

/// One hazard id and its sites, where the census records them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HazardSites {
    pub id: String,
    #[serde(default)]
    pub sites: Vec<HazardSite>,
}

/// The typed blockers of `program`'s uncertifiable rows that a formula holds, and the hazard table their `fired` ids name.
/// Computed from the final rows and the lookup's whole-program answers, so it is a function of the build and nothing else.
pub(crate) fn typed_blockers(
    program: &Program,
    lookup: &SemanticLookup<'_>,
) -> (Vec<BlockedRow>, Vec<HazardSites>) {
    use crate::base::contributions::{handed_root_cause, impostor_cause, impostor_veto};
    let mut rows = Vec::<BlockedRow>::new();
    let mut ids = BTreeSet::<String>::new();
    let hazard = |hazard| lookup.hazard_present(hazard);
    // The files by path, as the lookup's last slot of a path holds them, built at the first row that needs one.
    let mut files = None::<BTreeMap<&str, &FileFacts>>;
    // `handed_root_patch` asked of a name once: it reads the program's hazards and the name, never the call.
    let mut handed = BTreeMap::<&str, Option<Vec<String>>>::new();
    for defect in program
        .static_defects
        .iter()
        .filter(|defect| defect.uncertain)
    {
        let Some(kind) = blocked_kind(&defect.analysis_context) else {
            continue;
        };
        let (formula, fired) = if kind == crate::blockers::STANDARD_MEMBER_IMPOSTOR {
            // The row stands at the callee of a withheld standard call.
            let (Ok(start), Ok(end)) = (
                u32::try_from(defect.location.start_byte),
                u32::try_from(defect.location.end_byte),
            ) else {
                continue;
            };
            let at = Span::new(start, end);
            let files = files.get_or_insert_with(|| {
                lookup
                    .files()
                    .iter()
                    .map(|file| (file.path.as_str(), file))
                    .collect()
            });
            let Some(file) = files.get(defect.location.path.as_ref()).copied() else {
                continue;
            };
            let Some(name) = lookup.resolved_standard_call_name(file, at) else {
                continue;
            };
            let by_handed = handed
                .entry(name)
                .or_insert_with(|| handed_root_cause(name, &hazard))
                .clone();
            if let Some(fired) = by_handed {
                ("handed-root-patch", fired)
            } else if impostor_veto(file, at, name, hazard, |member| {
                lookup.property_may_be_defined_reflectively_as(member)
            }) {
                ("impostor-veto", impostor_cause(name, &hazard))
            } else {
                continue;
            }
        } else if kind == crate::blockers::TUPLE_ITERATION_WRITTEN {
            let causes = lookup.array_slots_withdrawn_causes();
            if causes.is_empty() {
                continue;
            }
            (
                "array-slots-withdrawn",
                causes.into_iter().map(str::to_owned).collect(),
            )
        } else {
            continue;
        };
        ids.extend(fired.iter().cloned());
        rows.push(BlockedRow {
            location: defect.location.clone(),
            blocked_by: BlockedBy {
                kind: kind.to_owned(),
                scope: Some(PROGRAM_SCOPE.to_owned()),
                formula: Some(formula.to_owned()),
                fired,
                missing: None,
            },
        });
    }
    rows.sort_by(|left, right| {
        (
            left.location.path.as_ref(),
            left.location.start_byte,
            left.location.end_byte,
            &left.blocked_by.kind,
        )
            .cmp(&(
                right.location.path.as_ref(),
                right.location.start_byte,
                right.location.end_byte,
                &right.blocked_by.kind,
            ))
    });
    rows.dedup();
    let sites = if ids.is_empty() {
        BTreeMap::new()
    } else {
        crate::base::contributions::hazard_sites(lookup.files(), &ids)
    };
    let hazards = ids
        .into_iter()
        .map(|id| {
            let sites = sites
                .get(&id)
                .into_iter()
                .flatten()
                .map(|site| HazardSite {
                    path: site.path.clone(),
                    start: site.start,
                    end: site.end,
                })
                .collect();
            HazardSites { id, sites }
        })
        .collect();
    (rows, hazards)
}
