//! Source-analysis summary vocabulary for callbacks, returns and scheduling.
//!
//! Local and dependency-source analysis produce these shared rows. They carry
//! no package-document view, receipt or wire-document representation.

use std::collections::BTreeMap;

use serde::Serialize;

/// Which protocol of the caller's value an `invoke` operation runs.
///
/// A call is the historical meaning of every `invoke`. The other
/// four are the non-call invocations of caller-supplied code `semantic-model.md`
/// § callbacks names: a property read that may run a getter or a proxy trap
/// (`Get`), the iteration protocol (`Iterate`), ToPrimitive (`Coerce`), and
/// `Symbol.hasInstance` (`HasInstance`). Two explicit value enumerations
/// additionally distinguish which property values are obtained; their entry
/// bounds never establish every getter's execution. Each runs whatever the caller's value
/// carries, at the call, on the caller's stack, in the caller's tracking
/// context. Historical argument protocol items are always
/// `ambient-at-execution`. A catalogued callback-result use states its exact
/// execution point, owner and tracking independently of the producer.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InvokeProtocol {
    Call,
    Get,
    Iterate,
    Coerce,
    HasInstance,
    /// One for-in value-copy enumeration entered on every call. Own keys
    /// precede inherited enumerable string keys. This does not guarantee
    /// reaching a key after an earlier getter throws or mutates the object.
    GetEnumerableStringValues,
    /// At most one CopyDataProperties occurrence, after an unproved prefix.
    /// Own enumerable strings and symbols participate; inherited keys do not.
    GetOwnEnumerableValues,
}

impl InvokeProtocol {
    #[must_use]
    pub const fn is_value_enumeration(self) -> bool {
        matches!(
            self,
            Self::GetEnumerableStringValues | Self::GetOwnEnumerableValues
        )
    }
}

/// Whether a `for…in` value copy is guaranteed to Get one getter.
///
/// A preceding accessor that throws is an abrupt completion, which the
/// statically-present-code premise puts out of scope (owner decision
/// 2026-10-13, ADR 0274). A preceding accessor that may delete a later key or
/// replace its descriptor still breaks the guarantee: it is counted in
/// `preceding_mutation_hazards`. `receiver_exact` must come from a
/// descriptor-aware, non-escaped allocation proof, not a TypeScript object
/// type. Methods held as data are never called by this use.
#[must_use]
pub fn enumeration_get_is_guaranteed(
    protocol: InvokeProtocol,
    receiver_exact: bool,
    preceding_mutation_hazards: usize,
) -> bool {
    protocol == InvokeProtocol::GetEnumerableStringValues
        && receiver_exact
        && preceding_mutation_hazards == 0
}

#[cfg(test)]
mod enumeration_tests {
    use super::*;

    #[test]
    fn getter_strength_requires_receiver_and_a_hazard_free_prefix() {
        let protocol = InvokeProtocol::GetEnumerableStringValues;
        assert!(enumeration_get_is_guaranteed(protocol, true, 0));
        assert!(!enumeration_get_is_guaranteed(protocol, false, 0));
        assert!(!enumeration_get_is_guaranteed(protocol, true, 1));
        assert!(!enumeration_get_is_guaranteed(
            InvokeProtocol::GetOwnEnumerableValues,
            true,
            0
        ));
        assert!(!enumeration_get_is_guaranteed(InvokeProtocol::Get, true, 0));
        assert!(!enumeration_get_is_guaranteed(
            InvokeProtocol::Call,
            true,
            0
        ));
    }
}

/// When a `tracked` callback row runs, relative to the export returning.
///
/// `execution: "tracked"` is an *attribution* word: it says the runtime
/// subscribes the callback's reads, and says nothing about whether the export
/// has already run it. `inline` and `deferred` carry their schedule in the word
/// itself; `tracked` cannot, because 1.x `createMemo` runs its compute during
/// the creating call while 1.x `createEffect` queues it, and 2.0 disagrees with
/// 1.x on `createEffect`. The schedule is
/// [`solid_dialect::Dialect::tracked_callback_timing`]'s to answer, and it is
/// carried here rather than re-derived from the word — a consumer that reads
/// "queued" out of "tracked" publishes a promise the runtime breaks, which is
/// what `mergeProps` and `createMemo` rows did before this field existed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallbackSchedule {
    /// The callback has run by the time the export returns.
    SameStack,
    /// The export queued it; it has not run when the export returns.
    Queued,
    /// The runtime hands it to an external scheduler.
    External,
    /// The dialect states no timing for this slot, so no schedule word is
    /// honest. The emitted operation carries no execution point at all rather
    /// than a guessed one.
    Unestablished,
    /// ADR 0139, and only on a `deferred` row: the export keeps the callable
    /// only in the value it returns (for a construction, the instance), and
    /// it runs later, on the stack of code that invokes it through that value.
    /// The row is a deferred invocation in every pass that models one; the
    /// variant exists so a `result-access` summary row stays distinct from an
    /// ordinary deferred invocation.
    ResultAccess,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallbackSummary {
    pub parameter: usize,
    pub execution: String,
    /// The schedule of a `tracked` row, where the producer established one.
    /// `None` is a producer that did not compute a schedule for this row and
    /// leaves the consumer's historical default in place; it is meaningless
    /// for `inline` and for `deferred`, whose word already carries the
    /// schedule -- except [`CallbackSchedule::ResultAccess`], which only a
    /// `deferred` row carries and which names ADR 0139's retention event.
    pub schedule: Option<CallbackSchedule>,
    /// Runtime arguments supplied when this callback is invoked. `null`
    /// preserves an unmodeled ordinary value at that position; a structured
    /// descriptor uses the same bounded accessor/store/tuple/object vocabulary
    /// as exported returns.
    pub arguments: Vec<Option<ReturnSummary>>,
    /// The owner context in which the runtime invokes this callback. Missing
    /// means the row describes timing only; consumers must keep
    /// the existing fail-closed owner behavior for that callback.
    pub owner: Option<String>,
    /// Whether the callback is proven to run with no listener of the caller's
    /// current, defined per execution word. `true` publishes
    /// `tracking: "untracked"`; `false` publishes `ambient-at-execution`, which
    /// claims nothing about the listener and is the fail-closed answer.
    ///
    /// - `inline`: a `Detaching` wrapper (`untrack`, `createRoot`,
    ///   `runWithOwner`, a package row that states the same) stands between the
    ///   export and the callback. A bare `fn()` does not clear:
    ///   `@solid-primitives/utils`' `access` is `typeof v === "function" ? v() : v`
    ///   and its row once said `untracked` like `untrack`'s did.
    /// - `deferred`: the deferral is proven to run the callback on a fresh stack
    ///   or with the listener cleared -- a reviewed host queue
    ///   (`runtime_semantics::FRESH_STACK_SCHEDULERS`), a dialect deferred slot
    ///   the dialect states untracked, a clearing wrapper *inside* the deferral,
    ///   or a dependency's row that states one. "Runs after the export returns"
    ///   is not that proof: a returned closure (`safe`, `pipe`), a bound
    ///   function and an event listener all run on their caller's stack, inside
    ///   whatever computation that caller is in.
    /// - `tracked`: always `false`. The word is the attribution claim, and a
    ///   clearing wrapper outside a tracking one cannot undo its subscription.
    ///
    /// A bool rather than an enum because the state space is exactly
    /// (word x proven-or-not): no consumer distinguishes *why* a clearing was
    /// proven, and the one invalid pair, `tracked` with `true`, is excluded at
    /// both constructors -- the chain composition and
    /// [`CallbackSummary::clears_tracking_from`].
    pub clears_tracking: bool,
    /// Which protocol of the caller's value this row invokes. `Call` is every
    /// row an analysis pass writes; the others are non-call invocations (a
    /// property read, iteration, coercion or `hasInstance` of the argument),
    /// and they are **not** inline invocations of a callable: no consumer
    /// pass may read one as a call of the argument. See
    /// [`CallbackSummary::is_invocation`].
    pub protocol: InvokeProtocol,
    /// The member of the argument at [`Self::parameter`] this row invokes,
    /// outwards from the argument: empty for the argument itself, `["0"]` for
    /// `handler[0](…)` (item B of ways-to-improve § 3.3). A row with a path is
    /// a call of *that member*, never of the argument: see
    /// [`CallbackSummary::invokes_argument`] and
    /// [`CallbackSummary::invokes_member`].
    pub path: Vec<String>,
}

impl CallbackSummary {
    /// Whether this row is an invocation of the argument *as a callable* --
    /// the only kind of row an interprocedural or owner pass models.
    ///
    /// Every pass that reads rows to build edges, invoked parameters, wrappers, owner
    /// edges or accessor arguments filters on this first. A property read or
    /// coercion of the caller's value runs that value's own traps at the call,
    /// on the caller's stack, in the caller's tracking context -- what the
    /// value's author wrote -- and today such a use of a non-callable argument
    /// raises no obligation at all, so ignoring the row loses nothing the
    /// consumer modelled.
    #[must_use]
    pub fn is_invocation(&self) -> bool {
        self.protocol == InvokeProtocol::Call
    }

    /// Whether this row is ADR 0139's `result-access` item: a `deferred`
    /// invocation of a callable the export keeps only in the value it
    /// returns. Every pass that models invocations reads it as the deferred
    /// row it is; only projection and re-emission ask.
    #[must_use]
    pub fn is_result_access(&self) -> bool {
        self.execution == "deferred" && self.schedule == Some(CallbackSchedule::ResultAccess)
    }

    /// Whether this row calls the argument at [`Self::parameter`] itself: an
    /// invocation with an empty path. Every pass that reads a row as "the
    /// value passed here is called" -- a call-graph edge to the argument, an
    /// invoked parameter, a wrapper, an owner edge, an execution role for the
    /// argument -- asks this, not [`Self::is_invocation`]: a member-path row
    /// calls a member of the argument, and reading it as a call of the
    /// argument would fold `callHandler(e, handlerProp)` as a call of
    /// `handlerProp`.
    #[must_use]
    pub fn invokes_argument(&self) -> bool {
        self.is_invocation() && self.path.is_empty()
    }

    /// Whether this row calls a member of the argument, at [`Self::path`]
    /// (item B of ways-to-improve § 3.3). A consumer folds such a row only
    /// when the argument written at the slot resolves that member exactly.
    #[must_use]
    pub fn invokes_member(&self) -> bool {
        self.is_invocation() && !self.path.is_empty()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ReturnSummary {
    pub kind: String,
    pub label: String,
    pub parameter: Option<usize>,
    pub elements: Vec<Option<ReturnSummary>>,
    pub properties: BTreeMap<String, ReturnSummary>,
}

#[cfg(test)]
mod tests;
