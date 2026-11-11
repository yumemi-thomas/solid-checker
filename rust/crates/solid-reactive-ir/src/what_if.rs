//! ADR 0950: what-if runs. `solid-checker-what-if --assume <hazard>` analyzes one project with the named hazard facts treated as absent, to predict what
//! proving them away would lift (the fix-9 veto formulas, `handed_root_patch`, `patch_vetoed`, `reached_patch`, and ADR 0947's `blockedBy.fired` ids).
//!
//! This module exists only with the cargo feature `what-if`, which only `make build-what-if` enables, into its own target directory and as its own binary:
//! a default build has neither this module nor the seam in [`crate::indexes::SemanticLookup::hazard_present`], so no flag, environment variable or input
//! can reach it. The feature is never in a `default` list, a workspace or CI.
//!
//! One seam: `hazard_present` answers `false` for an assumed hazard the program has, and records that it did (`flipped`). Hazard facts only: the answers of
//! other whole-program queries (the array-slots formula, the whole-index readers of the ordinal hazards) are not touched. A hazard answer is a whole-program
//! fact read by memoized computations, so the seam cannot say which finding read it: when any answer flipped, every finding of the run is tainted
//! (`premises: [{ "kind": "assumed", ... }]`, a top-level `assumed`), which is a superset of the findings the assumption touched. Every consumer refuses a
//! tainted report (scripts/lib/assumed.mjs and its callers); this binary has no daemon, no `--certify` and no text or ESLint format.

use std::collections::BTreeSet;
use std::sync::{Mutex, OnceLock};

use crate::base::Hazard;
use crate::base::contributions::hazard_of_id;

static ASSUMED: OnceLock<Vec<(String, Hazard)>> = OnceLock::new();
static FLIPPED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// Names the hazard facts to treat as absent for this process, once. Ids are those of `blockedBy.fired` (`OpenMemberWrite(*)`, `RootPatch`,
/// `StandardImpostor(forEach)`, `StandardPatch(.Symbol)`, ...); an unknown id is an error, never ignored.
///
/// # Errors
/// When an id names no hazard, or the process already assumed some.
pub fn assume(ids: &[String]) -> Result<(), String> {
    let mut parsed = Vec::<(String, Hazard)>::new();
    for id in ids {
        let hazard =
            hazard_of_id(id).ok_or_else(|| format!("--assume: `{id}` names no hazard fact"))?;
        if !parsed.iter().any(|(_, held)| *held == hazard) {
            parsed.push((id.clone(), hazard));
        }
    }
    ASSUMED
        .set(parsed)
        .map_err(|_| "--assume: the assumption of this process is already set".to_owned())
}

/// The seam's answer: whether `hazard` is assumed absent. Records it as flipped; the caller asks only for a hazard the program has.
pub(crate) fn assumed_absent(hazard: Hazard) -> bool {
    let Some((id, _)) = ASSUMED
        .get()
        .and_then(|assumed| assumed.iter().find(|(_, held)| *held == hazard))
    else {
        return false;
    };
    FLIPPED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(id.clone());
    true
}

/// ADR 0965: the kinds of the `Premise(kind)` ids the process assumed, in the order given.
pub(crate) fn premises() -> Vec<&'static str> {
    ASSUMED.get().map_or_else(Vec::new, |assumed| {
        assumed
            .iter()
            .filter_map(|(_, hazard)| match hazard {
                Hazard::Premise(kind) => Some(*kind),
                _ => None,
            })
            .collect()
    })
}

/// The ids the process assumed, in the order given.
#[must_use]
pub fn assumed_ids() -> Vec<String> {
    ASSUMED.get().map_or_else(Vec::new, |assumed| {
        assumed.iter().map(|(id, _)| id.clone()).collect()
    })
}

/// The assumed ids whose answer actually flipped (the program had the hazard), sorted. Empty: the run is the default build's.
#[must_use]
pub fn flipped() -> Vec<String> {
    FLIPPED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_id_is_an_error_whatever_else_is_named() {
        assert!(assume(&["NoSuchHazard".to_owned()]).is_err());
        assert!(assume(&["OpenMemberWrite(".to_owned()]).is_err());
        assert!(assume(&["RootPatch".to_owned(), "StandardPatch(nodot)".to_owned()]).is_err());
    }

    // One process, one assumption (the statics are the process's): the whole life of it in one test.
    #[test]
    fn the_seam_answers_absent_for_the_assumed_hazards_only_and_records_them() {
        assert!(!assumed_absent(Hazard::RootPatch), "nothing is assumed yet");
        assert!(flipped().is_empty());
        assume(&[
            "RootPatch".to_owned(),
            "OpenMemberWrite(*)".to_owned(),
            "RootPatch".to_owned(),
        ])
        .unwrap();
        assert_eq!(assumed_ids(), ["RootPatch", "OpenMemberWrite(*)"]);
        assert!(
            assume(&["StandardRootHanded".to_owned()]).is_err(),
            "assumed once per process"
        );
        assert!(assumed_absent(Hazard::RootPatch));
        assert!(!assumed_absent(Hazard::StandardRootHanded));
        assert_eq!(flipped(), ["RootPatch"]);
    }
}
