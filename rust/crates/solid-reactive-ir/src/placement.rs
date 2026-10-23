//! Why a site is unplaced: the blocker an [`ExecutionRole::Unknown`] answer
//! carries.
//!
//! A measurement annotation only. It never decides a role, a finding's kind or
//! its message; it names which missing proof left the role unknown, so an
//! uncertifiable obligation can be counted by cause (`--format json`,
//! `scripts/corpus-metrics.mjs`).
//!
//! The role classifiers return a bare [`ExecutionRole`], so the blocker is
//! recorded beside the answer rather than in it: each site that produces
//! `Unknown` calls [`unplaced`], and a caller that needs the blocker runs the
//! classification under [`traced`]. Propagating an `Unknown` a callee produced
//! keeps the callee's blocker.

use std::cell::Cell;

use serde::{Deserialize, Serialize};

use crate::ExecutionRole;

/// The missing proof that left a read, write or action unplaced.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlacementBlocker {
    /// The Solid compiler refused the file, so it has no compiler facts
    /// (ADR 0360).
    RefusedFile,
    /// An event handler a dispatching call may run on its own stack, outside
    /// the host's dispatch (ADR 0356, 0361, 0366).
    DispatchConditionalHandler,
    /// Code that may follow a suspension of its async function: a skippable
    /// `await`, a `for await`, an `await` in a loop (ADR 0354, 0357).
    PossibleSuspension,
    /// Code a component element's children hold, which the receiving
    /// component runs when it evaluates `children` (open-work section 3).
    ComponentChildren,
    /// A function that is a JSX attribute's value, run whenever the element
    /// or component it is handed to calls it.
    PropCallback,
    /// A function literal handed to a call not proven to invoke it during
    /// the call.
    CallbackUnknownConsumer,
    /// A function no resolved call, flow or proof invokes.
    NoResolvedCaller,
    /// A function whose invocations were followed and include a site that is
    /// itself unplaced.
    UnplacedCaller,
    /// A function with resolved callers in a program not declared closed,
    /// where invocation flow (ADR 0319) does not run.
    OpenProgram,
    /// A function with resolved callers that invocation flow did not place:
    /// a holder the walk cannot follow, or its budget.
    CallerFlowUnresolved,
    /// Invocations in roles that do not agree, one of them reporting
    /// (ADR 0319, 0323).
    RoleMix,
    /// An instance field initializer, which runs when an instance is
    /// constructed.
    InstanceField,
    /// A callback run on reads of a returned view, or wrapped in a memo of
    /// the primitive's own (rc.9 `omit`, 2.0 `merge`).
    ViewCallback,
    /// Whether a root wraps the callback is not audited on this release.
    UnauditedRelease,
    /// An effect's first run at a call site that forbids the write, not
    /// proven to settle during the call.
    FirstRunTiming,
    /// An effect apply whose invocation its defer options and compute
    /// dependencies leave unresolved (ADR 0302).
    DeferredApply,
    /// A JSX-returning function also handed to code that may call it plainly
    /// (ADR 0331).
    UndecidedComponent,
    /// A component only `Dynamic` renders, with no strict-read window
    /// (ADR 0138).
    WindowlessComponent,
    /// None of the above was recorded.
    Other,
}

impl PlacementBlocker {
    /// The kebab-case spelling the JSON report carries.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RefusedFile => "refused-file",
            Self::DispatchConditionalHandler => "dispatch-conditional-handler",
            Self::PossibleSuspension => "possible-suspension",
            Self::ComponentChildren => "component-children",
            Self::PropCallback => "prop-callback",
            Self::CallbackUnknownConsumer => "callback-unknown-consumer",
            Self::NoResolvedCaller => "no-resolved-caller",
            Self::UnplacedCaller => "unplaced-caller",
            Self::OpenProgram => "open-program",
            Self::CallerFlowUnresolved => "caller-flow-unresolved",
            Self::RoleMix => "role-mix",
            Self::InstanceField => "instance-field",
            Self::ViewCallback => "view-callback",
            Self::UnauditedRelease => "unaudited-release",
            Self::FirstRunTiming => "first-run-timing",
            Self::DeferredApply => "deferred-apply",
            Self::UndecidedComponent => "undecided-component",
            Self::WindowlessComponent => "windowless-component",
            Self::Other => "other",
        }
    }
}

thread_local! {
    /// The blocker of the most recent `Unknown` the running classification
    /// produced or propagated.
    static NOTED: Cell<Option<PlacementBlocker>> = const { Cell::new(None) };
}

/// [`ExecutionRole::Unknown`], recording why.
pub(crate) fn unplaced(blocker: PlacementBlocker) -> ExecutionRole {
    NOTED.set(Some(blocker));
    ExecutionRole::Unknown
}

/// Records `blocker` for an `Unknown` the caller returns by other means.
pub(crate) fn note(blocker: PlacementBlocker) {
    NOTED.set(Some(blocker));
}

/// The blocker recorded so far in the running classification.
#[cfg(test)]
fn noted() -> Option<PlacementBlocker> {
    NOTED.get()
}

/// Runs a role classification and returns its blocker beside it: `Some` only
/// for `Unknown`, [`PlacementBlocker::Other`] when no site recorded one.
///
/// What the classification recorded is scoped to it. An enclosing
/// classification keeps its own record, except that an `Unknown` answer
/// leaves this blocker recorded for it to propagate.
pub(crate) fn traced(
    classify: impl FnOnce() -> ExecutionRole,
) -> (ExecutionRole, Option<PlacementBlocker>) {
    let outer = NOTED.replace(None);
    let role = classify();
    let recorded = NOTED.get();
    let blocker =
        (role == ExecutionRole::Unknown).then(|| recorded.unwrap_or(PlacementBlocker::Other));
    NOTED.set(blocker.or(outer));
    (role, blocker)
}

/// Runs `compute` with the record saved and restored around it: a memo
/// computed during one classification records nothing into it.
pub(crate) fn isolated<T>(compute: impl FnOnce() -> T) -> T {
    let outer = NOTED.replace(None);
    let value = compute();
    NOTED.set(outer);
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traced_scopes_the_record_and_propagates_an_unknown() {
        note(PlacementBlocker::RoleMix);
        let (role, blocker) = traced(|| ExecutionRole::TrackedJsx);
        assert_eq!((role, blocker), (ExecutionRole::TrackedJsx, None));
        assert_eq!(noted(), Some(PlacementBlocker::RoleMix));
        let (role, blocker) = traced(|| unplaced(PlacementBlocker::RefusedFile));
        assert_eq!(role, ExecutionRole::Unknown);
        assert_eq!(blocker, Some(PlacementBlocker::RefusedFile));
        assert_eq!(noted(), Some(PlacementBlocker::RefusedFile));
        let (_, blocker) = traced(|| ExecutionRole::Unknown);
        assert_eq!(blocker, Some(PlacementBlocker::Other));
        isolated(|| note(PlacementBlocker::InstanceField));
        assert_eq!(noted(), Some(PlacementBlocker::Other));
    }

    #[test]
    fn every_blocker_serializes_as_its_spelling() {
        for blocker in [
            PlacementBlocker::RefusedFile,
            PlacementBlocker::DispatchConditionalHandler,
            PlacementBlocker::PossibleSuspension,
            PlacementBlocker::ComponentChildren,
            PlacementBlocker::PropCallback,
            PlacementBlocker::CallbackUnknownConsumer,
            PlacementBlocker::NoResolvedCaller,
            PlacementBlocker::UnplacedCaller,
            PlacementBlocker::OpenProgram,
            PlacementBlocker::CallerFlowUnresolved,
            PlacementBlocker::RoleMix,
            PlacementBlocker::InstanceField,
            PlacementBlocker::ViewCallback,
            PlacementBlocker::UnauditedRelease,
            PlacementBlocker::FirstRunTiming,
            PlacementBlocker::DeferredApply,
            PlacementBlocker::UndecidedComponent,
            PlacementBlocker::WindowlessComponent,
            PlacementBlocker::Other,
        ] {
            assert_eq!(
                serde_json::to_string(&blocker).unwrap(),
                format!("\"{}\"", blocker.as_str())
            );
        }
    }
}
