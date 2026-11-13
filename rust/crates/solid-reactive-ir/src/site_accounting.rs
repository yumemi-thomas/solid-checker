//! ADR 0936: closed accounting for strict reads, writes, actions, store writes
//! and `onFlush` calls (the other half of ADR 0933's proof log, run on the
//! normal path).
//!
//! A clean was a row's absence: every producer returned from a site it did not
//! understand the same way it returned from one it had proved harmless. The
//! site universe ([`crate::proof_universe`]) lists the sites from facts alone,
//! and each must end in one of
//!
//! - a row of the final program at it (a read, an async read, a write, an
//!   action: a silent role is the role classifier's decision, which is its own
//!   audit), an SC9012 census obligation, a defect or a violation;
//! - the compiler's discard of the code;
//! - a [`SiteSilence`] its producer named: a closed list of reasons, each the
//!   producer's own early return, recorded in the producer's result beside its
//!   rows ([`SiteEnds`]) so that a retained build, a per-file chunk reused from
//!   an earlier build and a speculative result a producer drops all behave as
//!   the rows do.
//!
//! A site that ends in none of them gets an uncertifiable row of the rule's own
//! code, blocker `<family>-unaccounted` (ADR 0680): never a violation, and it
//! yields to any row that stands. The check is as ADR 0886's for SC4001 and
//! ADR 0838's for the pending rules; the proof log writes the same sites and
//! endings, so the gate and the runtime agree.

use std::sync::Arc;

use solid_facts::FileFacts;
use solid_facts::core::Span;
use typefacts::Location;

use crate::pipeline::AnalysisContext;
use crate::proof_log::{self, Premise, PremiseKind};
use crate::proof_universe::{self, Endings, RemovedBy, Site, SiteCode};
use crate::{HashMap, HashSet, Program, StaticViolation};

macro_rules! site_silences {
    ($($(#[$meta:meta])* $variant:ident => $id:literal,)*) => {
        /// A typed, proved silence a producer names for a site it gave no row.
        /// The set is closed: a producer that ends a site silently names the
        /// reason here or the site gets an uncertifiable row. One variant per
        /// producer return, with the id the proof log has always written
        /// (`<module>.<function>.<end>`), so a lint can list the ends from the
        /// source.
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub(crate) enum SiteSilence {
            $($(#[$meta])* $variant,)*
        }

        impl SiteSilence {
            /// Every silence, for the tests that map each to its producer.
            #[cfg(test)]
            pub(crate) const ALL: &'static [Self] = &[$(Self::$variant,)*];

            /// The id the proof log writes for the end.
            pub(crate) const fn id(self) -> &'static str {
                match self {
                    $(Self::$variant => $id,)*
                }
            }
        }
    };
}

site_silences! {
    /// A call inside a function argument that a known primitive never invokes
    /// (an async source's transported call).
    AsyncSitesDormantCallbackArgument => "local_access.async_sites.dormant-callback-argument",
    /// The same, for a direct call.
    CallDormantCallbackArgument => "local_access.discover_call_as.dormant-callback-argument",
    /// A function handed to a carrier that runs it where no read rule reports.
    CallHandedSilent => "local_access.discover_call_as.handed-silent",
    /// A named function at a tracked position (an effect's compute).
    CallNamedAtTrackedPosition => "local_access.discover_call_as.named-at-tracked-position",
    /// A key a `merge` view serves from a plain source.
    MemberMergeViewPlain => "local_access.discover_member.merge-view-plain",
    /// A named function at a tracked position.
    MemberNamedAtTrackedPosition => "local_access.discover_member.named-at-tracked-position",
    /// A read in a function the program places elsewhere (its call edges).
    MemberPlacedElsewhere => "local_access.discover_member.placed-elsewhere",
    /// A prop every caller passes a static value.
    MemberPropsStatic => "local_access.discover_member.props-static",
    /// A component's `ref` prop called once to publish its handle.
    MemberRefPropCall => "local_access.discover_member.ref-prop-call",
    /// A store `then` Get the thenable protocol makes exempt.
    MemberStoreThenExempt => "local_access.discover_member.store-then-exempt",
    /// A view (`omit`, `merge`) hides the key the read names.
    MemberViewHidesKey => "local_access.discover_member.view-hides-key",
    /// Every reference of a helper is a call accounted at its own site.
    HelperCallsAccounted => "local_access.project_helper_prop_read.calls-accounted",
    /// An optimistic signal setter meets no owned-scope write guard.
    SetterOptimisticSignalUnguarded => "local_access.push_setter_write.optimistic-signal-unguarded",
    /// An optimistic store setter meets no owned-scope write guard.
    SetterOptimisticStoreUnguarded => "local_access.push_setter_write.optimistic-store-unguarded",
    /// A read in a function no call edge reaches, at a tracked position.
    UnreachedReadTrackedPosition => "local_access.unreached_read.named-at-tracked-position",
    /// A read in a function no call edge reaches, placed elsewhere.
    UnreachedReadPlacedElsewhere => "local_access.unreached_read.placed-elsewhere",
    /// A setter or action call that cannot run (module scope, or unreached).
    UnreachedWriteCannotRun => "local_access.unreached_write.cannot-run",
    /// `createMutable`: its proxy is designed to be written through.
    DirectCreateMutable => "shared_reactivity.direct_mutations.create-mutable",
    /// The written root holds one prop's value, not the props container.
    DirectDestructuredPropValue => "shared_reactivity.direct_mutations.destructured-prop-value",
    /// The write has an exempt standing (the store's own setter, a raw value).
    DirectExemptStanding => "shared_reactivity.direct_mutations.exempt-standing",
    /// A logical assignment proven not to write.
    DirectLogicalAssignmentSkips => "shared_reactivity.direct_mutations.logical-assignment-skips",
    /// The store is shallow: a write below its root lands.
    DirectShallowStoreRoot => "shared_reactivity.direct_mutations.shallow-store-root",
    /// A nullish link on the chain: TypeScript reports it.
    DirectTscNullishLink => "shared_reactivity.direct_mutations.tsc-nullish-link",
    /// TypeScript rejects the write to the root.
    DirectTscRejectsRootWrite => "shared_reactivity.direct_mutations.tsc-rejects-root-write",
    /// A `delete` with an exempt standing.
    DeleteExemptStanding => "shared_reactivity.no_store_delete.exempt-standing",
    /// A `delete` through a shallow store root.
    DeleteShallowStoreRoot => "shared_reactivity.no_store_delete.shallow-store-root",
    /// A `delete` through a nullish link: TypeScript's.
    DeleteTscNullishLink => "shared_reactivity.no_store_delete.tsc-nullish-link",
    /// A type assertion on the `delete` operand: TS2703.
    DeleteTscOperandAssertion => "shared_reactivity.no_store_delete.tsc-operand-assertion",
    /// TypeScript rejects the `delete`.
    DeleteTscRejectsDelete => "shared_reactivity.no_store_delete.tsc-rejects-delete",
    /// An in-place array method call with an exempt standing.
    MutatorExemptStanding => "shared_reactivity.no_store_mutator_call.exempt-standing",
    /// The callee resolves to a method that is not the `Array` one.
    MutatorNotArrayMethod => "shared_reactivity.no_store_mutator_call.not-array-method",
    /// A mutator call through a shallow store root.
    MutatorShallowStoreRoot => "shared_reactivity.no_store_mutator_call.shallow-store-root",
    /// A mutator call through a nullish link: TypeScript's.
    MutatorTscNullishLink => "shared_reactivity.no_store_mutator_call.tsc-nullish-link",
    /// A write through a prop every caller passes a static value: it lands on a
    /// plain object.
    ContainerPropStatic => "shared_reactivity.push_unproven_container_write.props-static",
    /// A write into a container a caller handed in, which holds a plain value
    /// (a literal, a function, a `new` object) on every route (ADR 0919).
    HandedRootPlain => "shared_reactivity.handed_container_write.plain-on-every-route",
    /// Every caller of the function whose parameter is the root was enumerated
    /// and none hands a store.
    FlowCallersPassNoStore => "shared_reactivity.push_parameter_store_flow.callers-pass-no-store",
    /// A write through a parameter that is a shallow store root.
    FlowShallowStoreRoot => "shared_reactivity.push_parameter_store_flow.shallow-store-root",
    /// A write through a parameter's nullish link: TypeScript's.
    FlowTscNullishLink => "shared_reactivity.push_parameter_store_flow.tsc-nullish-link",
    /// An effect's apply is proven never to run (`effect_execution`, ADR 0590):
    /// the pass removed the row.
    ApplyNeverRuns => "effect_execution.gate_diagnostics.apply-never-runs",
    /// A dispatch obligation of an exported helper in a closed program, every way
    /// into which is a call expression the graph resolves: the pass removed the row.
    ClosedProgramExportDischarge => "attribution.discharge_closed_program_export_dispatch.closed-callers",
    /// A use of a parameter nothing resolves, at an argument that holds a primitive
    /// on every run (ADR 0320), in a closed program: the pass removed the row.
    ArgumentPrimitiveDischarge => "attribution.discharge_closed_program_export_dispatch.argument-primitive",
    /// A member dispatch on an argument that is a built-in value or a literal whose
    /// own member is a primitive (ADRs 0211-0214), in a closed program: the pass
    /// removed the row.
    ArgumentMemberPrimitiveDischarge => "attribution.discharge_closed_program_export_dispatch.argument-member-primitive",
    /// A function the program places elsewhere (its call edges), read through
    /// a call: the summaries decide at the calls.
    CallPlacedElsewhere => "local_access.discover_call_as.placed-elsewhere",
    /// The classifier decided the call's role reports nothing (tracked JSX, a
    /// deferred callback, a handler), and the producer returned without a row.
    CallRoleDoesNotReport => "local_access.discover_call_as.role-does-not-report",
    /// A read in a setter's updater: the setter call's write is a proven
    /// violation (`ownedWrite` proven off), so the guard throws before the
    /// updater runs (ADR 0932). The premise is that setter call.
    CallSetterGuardThrowsFirst => "local_access.discover_call_as.setter-guard-throws-first",
    /// A read in a setter's updater, where the setter call's role reports
    /// nothing: the updater runs at the call, in a place the rule allows.
    CallSetterCallbackRoleSilent => "local_access.discover_call_as.setter-callback-role-silent",
    /// A read at module scope, which no component or effect body owns: the
    /// rc.21 runtime installs strict-read contexts only in those.
    NoWindowModuleScope => "local_access.no_window.module-scope",
    /// A read past an `await` its function reaches on every path: a later
    /// task, off every caller's stack (ADR 0320).
    NoWindowAfterAwait => "local_access.no_window.after-await",
    /// A read in a function nothing enters (a closed program's export nothing
    /// names, a private function nothing references): it never runs.
    NoWindowNothingEnters => "local_access.no_window.nothing-enters",
    /// A store Get under a key the checker cannot spell, at a site whose role
    /// reports nothing.
    MemberRoleDoesNotReport => "local_access.discover_member.role-does-not-report",
    /// `onFlush` where the write role allows it (an effect's apply, a leaf, a
    /// handler): no owner rejects the call.
    OnFlushRoleAllows => "static_api.on-flush.role-allows",
    /// An owner-call site (`onCleanup`, a creation) that ADR 0458's query proves no execution runs (batch 11, slice 6).
    OwnerCallNeverRuns => "leaf_universe.account.never-runs",
    /// An owner-call site outside every function (and no instance field's initializer): module evaluation runs it on a stack of its own, in no leaf
    /// owner's callback and on no effect's apply.
    OwnerCallModuleScope => "leaf_universe.account.module-scope",
    /// An owner-call site the owner graph proves no leaf owner runs: the function's context has an edge, no leaf bit, every entry is held, and no package the
    /// build does not read may run it.
    LeafOwnerAbsent => "leaf_universe.account.owner-context-no-leaf",
    /// A creation the dialect says throws on an effect's apply, in a function the owner graph gives an owner on every entry, under the same conditions: the
    /// callback's guard needs no owner.
    OwnerCurrent => "leaf_universe.account.owner-context-current",
    /// The project names no primitive that opens a leaf scope (and no package the build does not read may run the function), so no leaf owner exists to
    /// forbid the call.
    LeafNoOpenerInProgram => "leaf_universe.account.no-opener-in-program",
    /// A creation the dialect says throws on an effect's apply, written where the role engine places it off every effect's apply (a handler, a deferred
    /// callback, a directive apply, rendering, module code, discarded code).
    EffectCreationRunsOffApply => "leaf_universe.account.effect-creation-runs-off-apply",
    /// A creation the dialect says throws on an effect's apply, inside the callback of an exact owner-setting primitive (`runWithOwner`, `createRoot`, a
    /// computation's compute): an owner is current.
    EffectCreationOwnerSetAround => "leaf_universe.account.effect-creation-owner-set-around",
}

impl SiteSilence {
    /// The codes the silence is a proof for, the producer's own: a silence answers the code of the table its site is in, never another code of a site that
    /// shares the span (batch 11, slice 4). One arm per variant, no wildcard: a new silence names its codes or the build fails. A silence that removes a
    /// row (`removed`, `removed_with`) names none here: it answers the codes the removed row's ending answered ([`SiteEnd::answers`]).
    pub(crate) const fn codes(self) -> &'static [SiteCode] {
        use SiteCode::{
            ActionCall, CleanupForbidden, EffectCreation, OnFlush, PrimitiveForbidden,
            ReactiveWrite, StoreWrite, StrictRead,
        };
        match self {
            // The reads: an accessor's call, a store or props member Get.
            Self::AsyncSitesDormantCallbackArgument
            | Self::CallDormantCallbackArgument
            | Self::CallHandedSilent
            | Self::CallNamedAtTrackedPosition
            | Self::MemberMergeViewPlain
            | Self::MemberNamedAtTrackedPosition
            | Self::MemberPlacedElsewhere
            | Self::MemberPropsStatic
            | Self::MemberRefPropCall
            | Self::MemberStoreThenExempt
            | Self::MemberViewHidesKey
            | Self::HelperCallsAccounted
            | Self::UnreachedReadTrackedPosition
            | Self::UnreachedReadPlacedElsewhere
            | Self::CallPlacedElsewhere
            | Self::CallRoleDoesNotReport
            | Self::CallSetterGuardThrowsFirst
            | Self::CallSetterCallbackRoleSilent
            | Self::NoWindowModuleScope
            | Self::NoWindowAfterAwait
            | Self::NoWindowNothingEnters
            | Self::MemberRoleDoesNotReport => &[StrictRead],
            // A setter that meets no owned-scope write guard: the owned-scope write only. An optimistic setter of an async source takes the other
            // branch of `push_setter_write` (SC2009) before it reaches these.
            Self::SetterOptimisticSignalUnguarded | Self::SetterOptimisticStoreUnguarded => {
                &[ReactiveWrite]
            }
            // A setter or action call outside every function (module initialization has no owner) or in a function nothing reaches: the codes that
            // need an owner. An optimistic write before the load needs none, so a call at module scope keeps that code.
            Self::UnreachedWriteCannotRun => &[ReactiveWrite, ActionCall],
            // The store writes: an assignment, a `delete`, an in-place array method, through a store or props root.
            Self::DirectCreateMutable
            | Self::DirectDestructuredPropValue
            | Self::DirectExemptStanding
            | Self::DirectLogicalAssignmentSkips
            | Self::DirectShallowStoreRoot
            | Self::DirectTscNullishLink
            | Self::DirectTscRejectsRootWrite
            | Self::DeleteExemptStanding
            | Self::DeleteShallowStoreRoot
            | Self::DeleteTscNullishLink
            | Self::DeleteTscOperandAssertion
            | Self::DeleteTscRejectsDelete
            | Self::MutatorExemptStanding
            | Self::MutatorNotArrayMethod
            | Self::MutatorShallowStoreRoot
            | Self::MutatorTscNullishLink
            | Self::ContainerPropStatic
            | Self::HandedRootPlain
            | Self::FlowCallersPassNoStore
            | Self::FlowShallowStoreRoot
            | Self::FlowTscNullishLink => &[StoreWrite],
            Self::OnFlushRoleAllows => &[OnFlush],
            // The owner-call universe (slice 6): a site no execution runs, or outside every function, ends every code it can raise; the owner-graph and
            // census proofs answer the leaf owner's two codes, the effect proofs the effect callback's.
            Self::OwnerCallNeverRuns | Self::OwnerCallModuleScope => {
                &[CleanupForbidden, PrimitiveForbidden, EffectCreation]
            }
            Self::LeafOwnerAbsent | Self::LeafNoOpenerInProgram => {
                &[CleanupForbidden, PrimitiveForbidden]
            }
            Self::OwnerCurrent
            | Self::EffectCreationRunsOffApply
            | Self::EffectCreationOwnerSetAround => &[EffectCreation],
            // The passes that remove a row: the codes the removed row's ending answered.
            Self::ApplyNeverRuns
            | Self::ClosedProgramExportDischarge
            | Self::ArgumentPrimitiveDischarge
            | Self::ArgumentMemberPrimitiveDischarge => &[],
        }
    }

    /// Whether the silence names the row it removed instead of codes: it is recorded only by [`SiteEnds::removed`] and [`SiteEnds::removed_with`].
    pub(crate) const fn removes_a_row(self) -> bool {
        matches!(
            self,
            Self::ApplyNeverRuns
                | Self::ClosedProgramExportDischarge
                | Self::ArgumentPrimitiveDischarge
                | Self::ArgumentMemberPrimitiveDischarge
        )
    }
}

/// `SOLID_CHECKER_IR_ABLATE_SILENCE=<id>[,<id>...]` (or `all`): the named
/// silences are not recorded, so the sites they end have no ending and get an
/// uncertifiable row. A test and measurement hook (the mutation test, and the
/// ranking of what stands on each silence alone), read once; unset, it costs a
/// load. It can only add uncertifiable rows.
fn ablated(silence: SiteSilence) -> bool {
    ablated_id(silence.id())
}

/// [`ablated`] for any typed ending's id: the passes that keep their own endings (`flush_accounting`) read the same switch.
pub(crate) fn ablated_id(id: &str) -> bool {
    static ABLATED: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let list = ABLATED.get_or_init(|| {
        std::env::var("SOLID_CHECKER_IR_ABLATE_SILENCE")
            .map(|value| {
                value
                    .split(',')
                    .map(|id| id.trim().to_owned())
                    .filter(|id| !id.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    });
    !list.is_empty() && list.iter().any(|listed| listed == "all" || listed == id)
}

/// One typed silence a producer recorded: the site it ended, the reason, and
/// the facts the reason rests on (each names the code it is about).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SiteEnd {
    path: Arc<str>,
    site: Span,
    silence: SiteSilence,
    premises: Vec<Premise>,
    /// The ending of the row a pass removed, for a silence that names it ([`SiteSilence::removes_a_row`]).
    removed: Option<RemovedBy>,
}

impl SiteEnd {
    fn location(&self) -> Location {
        self.site.location(Arc::clone(&self.path))
    }

    /// Whether the end is a proof for `code` of a site at its span: the codes the silence names, or the codes the removed row's ending would have answered
    /// (one function for that, [`proof_universe::credits`]).
    fn answers(&self, policy: Option<&solid_dialect::CodePolicy>, code: SiteCode) -> bool {
        match &self.removed {
            Some(by) => proof_universe::credits(policy, code, &by.by()),
            None => self.silence.codes().contains(&code),
        }
    }
}

/// The silences a producer recorded beside its rows. An end whose fact has no
/// span is a label, not a fact: nothing is recorded and the site stays
/// unaccounted for (ADR 0933).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SiteEnds(Vec<SiteEnd>);

impl SiteEnds {
    /// An end on one premise of `kind` about the code at `at()`: the place the
    /// fact lives (a carrier call, a props parameter, a function), not the site.
    pub(crate) fn end_at(
        &mut self,
        file: &FileFacts,
        site: Span,
        silence: SiteSilence,
        kind: PremiseKind,
        at: impl FnOnce() -> Option<Span>,
    ) {
        self.end_at_path(file.path.shared(), site, silence, kind, at);
    }

    fn end_at_path(
        &mut self,
        path: Arc<str>,
        site: Span,
        silence: SiteSilence,
        kind: PremiseKind,
        at: impl FnOnce() -> Option<Span>,
    ) {
        if let Some(span) = at() {
            self.push(
                path,
                site,
                silence,
                vec![Premise::at(kind, Some(span))],
                None,
            );
        }
    }

    /// An end on every premise it rests on: the premises that name no code are
    /// dropped, and with none left nothing is recorded.
    pub(crate) fn end_with(
        &mut self,
        file: &FileFacts,
        site: Span,
        silence: SiteSilence,
        premises: impl FnOnce() -> Vec<Premise>,
    ) {
        self.end_with_path(file.path.shared(), site, silence, premises);
    }

    fn end_with_path(
        &mut self,
        path: Arc<str>,
        site: Span,
        silence: SiteSilence,
        premises: impl FnOnce() -> Vec<Premise>,
    ) {
        let mut premises = premises();
        premises.retain(|premise| premise.at.is_some());
        if !premises.is_empty() {
            self.push(path, site, silence, premises, None);
        }
    }

    /// An end for a row a pass removed from the draft, on the one fact it
    /// removed it on, about the code of the row itself. `by` is what the removed row was (a read, a violation of a rule, an SC9012): the site ends for the
    /// codes that ending answered and for no other (batch 11, slice 4).
    pub(crate) fn removed(
        &mut self,
        location: &Location,
        silence: SiteSilence,
        kind: PremiseKind,
        by: RemovedBy,
    ) {
        self.removed_with(location, silence, &[kind], by);
    }

    /// `removed` for a removal that rests on several facts, each about the code of the row itself.
    pub(crate) fn removed_with(
        &mut self,
        location: &Location,
        silence: SiteSilence,
        kinds: &[PremiseKind],
        by: RemovedBy,
    ) {
        let (Ok(start), Ok(end)) = (
            u32::try_from(location.start_byte),
            u32::try_from(location.end_byte),
        ) else {
            return;
        };
        let site = Span::new(start, end);
        self.push(
            Arc::clone(&location.path),
            site,
            silence,
            kinds
                .iter()
                .map(|kind| Premise::at(*kind, Some(site)))
                .collect(),
            Some(by),
        );
    }

    fn push(
        &mut self,
        path: Arc<str>,
        site: Span,
        silence: SiteSilence,
        premises: Vec<Premise>,
        removed: Option<RemovedBy>,
    ) {
        if ablated(silence) {
            return;
        }
        debug_assert_eq!(
            silence.removes_a_row(),
            removed.is_some(),
            "{silence:?}: a silence that removes a row names the row's ending, any other names codes"
        );
        self.0.push(SiteEnd {
            path,
            site,
            silence,
            premises,
            removed,
        });
    }

    /// How many ends are recorded: a producer compares it before and after a
    /// return to know whether it named one.
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    /// An end for a site whose role the classifier decided reports nothing (it is
    /// recorded only for such a role: an unplaced one is an obligation, a
    /// reporting one a row). The fact is the decision at the site itself, or at
    /// `premise_at` where the role came from another place (a setter call).
    pub(crate) fn role_silent(
        &mut self,
        file: &FileFacts,
        site: Span,
        silence: SiteSilence,
        role: crate::ExecutionRole,
        premise_at: Span,
    ) {
        if role.reports_untracked_read() || role.reports_unplaced() {
            return;
        }
        self.end_at(file, site, silence, PremiseKind::RoleDoesNotReport, || {
            Some(premise_at)
        });
    }

    /// `other`'s ends after this one's.
    pub(crate) fn extend(&mut self, other: &Self) {
        self.0.extend(other.0.iter().cloned());
    }

    /// `other`'s ends after this one's, moved.
    pub(crate) fn append(&mut self, other: Self) {
        self.0.extend(other.0);
    }

    #[cfg(test)]
    pub(crate) fn silences(&self) -> Vec<SiteSilence> {
        self.0.iter().map(|end| end.silence).collect()
    }

    /// The silences by the span they name, by file.
    fn named(&self) -> HashMap<(&str, u32, u32), Vec<&SiteEnd>> {
        let mut named: HashMap<(&str, u32, u32), Vec<&SiteEnd>> = HashMap::default();
        for end in &self.0 {
            named
                .entry((end.path.as_ref(), end.site.start, end.site.end))
                .or_default()
                .push(end);
        }
        named
    }

    /// Writes each end as a `silent` line (the proof log's named ends), with the codes it answers under the dialect's policy.
    fn log(&self, dialect: &dyn solid_dialect::Dialect) {
        for end in &self.0 {
            let codes = SiteCode::ALL
                .iter()
                .copied()
                .filter(|code| {
                    end.answers(dialect.code_policy(code.runtime_code()).as_ref(), *code)
                })
                .map(SiteCode::name)
                .collect::<Vec<_>>();
            proof_log::silent_codes(
                "end",
                &end.location(),
                end.silence.id(),
                &end.premises,
                &codes,
            );
        }
    }
}

/// What a row a pass removes was, as an ending: the codes of the site universe it answered ([`RemovedBy`]). `None` for a row no site of the universe ends on
/// (a missing owner, a leaf operation): the removal is not recorded, as before. One impl per row type, so a new table the gate retains names its answer.
pub(crate) trait RemovedRow {
    fn ended_by(&self) -> Option<RemovedBy>;
}

impl RemovedRow for crate::ReactiveRead {
    fn ended_by(&self) -> Option<RemovedBy> {
        RemovedBy::of(&proof_universe::READ_ROW)
    }
}

impl RemovedRow for crate::ReactiveWrite {
    fn ended_by(&self) -> Option<RemovedBy> {
        RemovedBy::of(&proof_universe::WRITE_ROW)
    }
}

impl RemovedRow for crate::ActionInvocation {
    fn ended_by(&self) -> Option<RemovedBy> {
        RemovedBy::of(&proof_universe::ACTION_ROW)
    }
}

impl RemovedRow for crate::AsyncRead {
    fn ended_by(&self) -> Option<RemovedBy> {
        RemovedBy::of(&proof_universe::ASYNC_READ_ROW)
    }
}

impl RemovedRow for crate::StaticDefect {
    fn ended_by(&self) -> Option<RemovedBy> {
        Some(RemovedBy::defect(self))
    }
}

impl RemovedRow for StaticViolation {
    fn ended_by(&self) -> Option<RemovedBy> {
        Some(RemovedBy::violation(self))
    }
}

impl RemovedRow for crate::OwnerRequirement {
    fn ended_by(&self) -> Option<RemovedBy> {
        None
    }
}

impl RemovedRow for crate::LeafOwnerOperation {
    /// The site of a removed operation ends for the codes the operation would have answered (batch 11, slice 6): a cleanup's, a creation's.
    fn ended_by(&self) -> Option<RemovedBy> {
        let by = proof_universe::leaf_operation_row(&self.kind);
        // A kind that answers no code of the universe (a flush, an observer guard, an unfollowed callback) names nothing.
        if matches!(by, proof_universe::EndedBy::Table { answers: [], .. }) {
            return None;
        }
        RemovedBy::of(&by)
    }
}

/// ADR 0942: a rule that leaves a site to another rule's claim ("SC3001's claim") ends it only where that rule has a row at the site. A hand-off
/// to a rule with no row there proves nothing: Solid may report the code, and no rule says so. The site then ends as any other that no
/// producer accounted for, an uncertifiable row of the handing rule's own code, never silence. One variant per hand-off.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Delegate {
    /// SC3001 (`leaf-owner-forbidden-call`): a site that runs under a leaf owner is the leaf owner's refusal, not an unowned run
    /// (SC4001's `ContextProof::LeafOwned`, ADR 0347's amendment).
    LeafOwnerForbiddenCall,
}

impl Delegate {
    /// The id the proof log writes for the hand-off.
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::LeafOwnerForbiddenCall => {
                "owner_emission.leaf-owned.delegated-to-leaf-owner-forbidden-call"
            }
        }
    }
}

/// The span of the row `to` holds at the call `call` of `path`, if it holds one: for SC3001, a leaf operation (a proven violation or an
/// obligation) located inside the call. A leaf scope whose callback could not be resolved (`UnresolvedCallback`) stands at the leaf call,
/// not at any site inside the callback, so it never answers for one.
pub(crate) fn delegate_row(
    to: Delegate,
    leaf_operations: &[crate::LeafOwnerOperation],
    path: &str,
    call: Span,
) -> Option<Span> {
    match to {
        Delegate::LeafOwnerForbiddenCall => leaf_operations.iter().find_map(|operation| {
            if matches!(
                operation.kind,
                crate::LeafOwnerOperationKind::UnresolvedCallback
            ) {
                return None;
            }
            // An operation reached through a helper stands at the helper call, with its own sites kept beside; one reported at its own
            // call has none, and is its own site.
            let sites = if operation.sites.is_empty() {
                std::slice::from_ref(&operation.location)
            } else {
                operation.sites.as_slice()
            };
            sites.iter().find_map(|site| {
                if site.path.as_ref() != path {
                    return None;
                }
                let start = u32::try_from(site.start_byte).ok()?;
                let end = u32::try_from(site.end_byte).ok()?;
                (call.start <= start && end <= call.end).then(|| Span::new(start, end))
            })
        }),
    }
}

/// The uncertifiable row of `code` at `site`: the code's home rule, its blocker.
fn unaccounted_row(file: &FileFacts, site: &Site, code: SiteCode) -> StaticViolation {
    let home = code.home();
    let text = file.source_text(site.at).unwrap_or("this site");
    let text = text.get(..text.len().min(48)).unwrap_or(text);
    StaticViolation {
        id: home.id.into(),
        rule: home.rule.into(),
        message: format!(
            "this {} (`{}`) has no row and no recorded proof that places it, so whether this rule applies here is not decided",
            home.noun,
            text.trim()
        ),
        hint: "No analysis produced a row for this site and none named a fact that rules it out; it is reported as an obligation until one does.".into(),
        location: site.at.location(file.path.shared()),
        analysis_context: home.blocker.into(),
        fixes: vec![],
        uncertain: true,
    }
}

/// Ends every code of every site of the universe in the final `program`, or gives it the uncertifiable row of the code's own rule. Run once per build,
/// after every producer and pass that adds or removes a row and before the reachability queries over the rows. A site ends for a code only by an
/// ending that answers that code (a row of its table, an SC9012, a finding of a rule the dialect's policy says claims it, the compiler's discard) or by a
/// silence that names it; what stands at the span for another code ends nothing here (batch 11, slice 4). Returns the universe it listed.
pub(crate) fn account<'a>(
    ctx: &AnalysisContext<'a>,
    program: &mut Program,
    ends: &SiteEnds,
) -> Vec<(&'a FileFacts, Site)> {
    let universe = proof_universe::sites(ctx);
    let named = ends.named();
    let dialect = ctx.dialect;
    let logging = proof_log::enabled();
    let mut rows = Vec::new();
    {
        let endings = Endings::of(program);
        let mut filed: HashSet<(&str, u32, u32, SiteCode)> = HashSet::default();
        for (file, site) in &universe {
            for &code in site.codes {
                let end = endings.end(dialect, file, site, code);
                if logging {
                    proof_universe::log_site(file, site, code, &end);
                }
                if end.credited.is_some() {
                    continue;
                }
                let path = file.path.as_str();
                let policy = dialect.code_policy(code.runtime_code());
                let silenced = std::iter::once(site.at)
                    .chain(site.alternates.iter().copied())
                    .any(|span| {
                        named
                            .get(&(path, span.start, span.end))
                            .is_some_and(|here| {
                                here.iter().any(|end| end.answers(policy.as_ref(), code))
                            })
                    });
                if !silenced && filed.insert((path, site.at.start, site.at.end, code)) {
                    rows.push(unaccounted_row(file, site, code));
                }
            }
        }
    }
    if logging {
        ends.log(dialect);
        // Batch 11, slice 0 (measurement only): the candidate sites of the rules with no universe, written beside the endings above; it returns nothing.
        if crate::proof_candidates::enabled() {
            crate::proof_candidates::log(ctx, &universe);
        }
    }
    // In the universe's order, after the rows the program holds: the findings
    // are ordered by place when they are projected.
    program.static_violations.extend(rows);
    // The universe is the one list of the build's sites: the passes that run after this one (batch 11, slice 9B) read it rather than list it again.
    universe
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_silence_has_a_unique_id_in_the_form_of_the_proof_log() {
        let mut seen = std::collections::BTreeSet::new();
        for silence in SiteSilence::ALL {
            let id = silence.id();
            assert!(seen.insert(id), "{silence:?} repeats the id {id}");
            let parts = id.split('.').collect::<Vec<_>>();
            assert!(
                parts.len() >= 2
                    && parts
                        .iter()
                        .all(|part| !part.is_empty() && !part.contains(char::is_whitespace)),
                "{silence:?}: `{id}` is not <module>.<function>.<end>"
            );
        }
    }

    /// The sources that name silences, by the module a silence's id begins with.
    const PRODUCERS: [(&str, &str); 6] = [
        ("local_access", include_str!("local_access.rs")),
        (
            "shared_reactivity",
            include_str!("upstream_compat/shared_reactivity.rs"),
        ),
        ("static_api", include_str!("static_api.rs")),
        ("effect_execution", include_str!("effect_execution.rs")),
        ("attribution", include_str!("attribution.rs")),
        ("leaf_universe", include_str!("owners/leaf_universe.rs")),
    ];

    /// ADR 0886's test of its emission arms, for these: every silence is named
    /// by the producer its id says, and every producer return that names one
    /// does so through the closed list (no `proof_log::end_*` is left to name a
    /// label).
    #[test]
    fn every_silence_is_named_by_the_producer_its_id_says() {
        for silence in SiteSilence::ALL {
            let module = silence.id().split('.').next().unwrap();
            let (_, source) = PRODUCERS
                .iter()
                .find(|(name, _)| *name == module)
                .unwrap_or_else(|| panic!("{silence:?}: no producer source for `{module}`"));
            let uses = source.matches(&format!("SiteSilence::{silence:?}")).count();
            assert!(
                uses > 0,
                "{silence:?} is named by no producer of `{module}`: a silence nobody records is a dead end"
            );
        }
        for (name, source) in PRODUCERS {
            assert!(
                !source.contains("proof_log::end_at(") && !source.contains("proof_log::end_with("),
                "{name} still names an end outside the closed list"
            );
        }
        for module in [
            include_str!("owners/owner_discovery.rs"),
            include_str!("pending_accounting.rs"),
            include_str!("flush_accounting.rs"),
        ] {
            // The accounting passes of ADR 0886 and ADR 0838 and the flush
            // universe (batch 11, owner-approved) keep their own typed endings
            // and their own universes.
            assert!(!module.contains("SiteSilence::"));
        }
    }

    #[test]
    fn an_end_without_a_premise_that_names_code_is_not_recorded() {
        let mut ends = SiteEnds::default();
        let path: Arc<str> = "/p/App.tsx".into();
        ends.end_at_path(
            Arc::clone(&path),
            Span::new(0, 5),
            SiteSilence::MemberPropsStatic,
            PremiseKind::PropsCallersStatic,
            || None,
        );
        ends.end_with_path(
            Arc::clone(&path),
            Span::new(0, 5),
            SiteSilence::CallHandedSilent,
            || vec![Premise::at(PremiseKind::CarrierPlacesRead, None)],
        );
        assert!(ends.silences().is_empty());
        ends.end_at_path(
            path,
            Span::new(0, 5),
            SiteSilence::MemberPropsStatic,
            PremiseKind::PropsCallersStatic,
            || Some(Span::new(1, 2)),
        );
        assert_eq!(ends.silences(), vec![SiteSilence::MemberPropsStatic]);
    }

    #[test]
    fn a_silence_names_a_site_by_its_span_in_its_file() {
        let mut ends = SiteEnds::default();
        ends.end_at_path(
            "/p/App.tsx".into(),
            Span::new(2, 9),
            SiteSilence::MemberRefPropCall,
            PremiseKind::RefPropCall,
            || Some(Span::new(2, 9)),
        );
        let named = ends.named();
        assert!(named.contains_key(&("/p/App.tsx", 2, 9)));
        assert!(!named.contains_key(&("/p/App.tsx", 2, 8)));
        assert!(!named.contains_key(&("/p/Other.tsx", 2, 9)));
    }

    fn leaf(
        kind: crate::LeafOwnerOperationKind,
        path: &str,
        start: u64,
        end: u64,
    ) -> crate::LeafOwnerOperation {
        crate::LeafOwnerOperation {
            kind,
            owner: "createTrackedEffect".into(),
            location: Location {
                path: path.into(),
                start_byte: start,
                end_byte: end,
            },
            fix: None,
            call_site_gate: None,
            uncertain: false,
            possible: false,
            via: None,
            identity_unproven: false,
            sites: Vec::new(),
            computation_error_unobserved: false,
            stop: None,
        }
    }

    #[test]
    fn a_hand_off_to_a_leaf_owner_needs_its_row_inside_the_call() {
        let call = Span::new(40, 60);
        let at = |operation| {
            delegate_row(
                Delegate::LeafOwnerForbiddenCall,
                &[operation],
                "/p/App.tsx",
                call,
            )
        };
        // A row at the call's callee, or at the whole call, is the leaf owner's claim.
        assert_eq!(
            at(leaf(
                crate::LeafOwnerOperationKind::Cleanup,
                "/p/App.tsx",
                40,
                49
            )),
            Some(Span::new(40, 49))
        );
        assert_eq!(
            at(leaf(
                crate::LeafOwnerOperationKind::Cleanup,
                "/p/App.tsx",
                40,
                60
            )),
            Some(Span::new(40, 60))
        );
        // An obligation (an uncertain row) is a row too: the rule has said something there.
        let mut uncertain = leaf(
            crate::LeafOwnerOperationKind::Primitive("createEffect".into()),
            "/p/App.tsx",
            40,
            60,
        );
        uncertain.uncertain = true;
        assert!(at(uncertain).is_some());
        // Not at the call: another call, a larger one holding it (the memo whose compute holds the site), another file.
        assert_eq!(
            at(leaf(
                crate::LeafOwnerOperationKind::Cleanup,
                "/p/App.tsx",
                61,
                70
            )),
            None
        );
        assert_eq!(
            at(leaf(
                crate::LeafOwnerOperationKind::Primitive("createMemo".into()),
                "/p/App.tsx",
                10,
                90
            )),
            None
        );
        assert_eq!(
            at(leaf(
                crate::LeafOwnerOperationKind::Cleanup,
                "/p/Other.tsx",
                40,
                60
            )),
            None
        );
        // An operation reached through a helper stands at the helper call, elsewhere: its own site is the row's place.
        let mut through_helper = leaf(
            crate::LeafOwnerOperationKind::Cleanup,
            "/p/App.tsx",
            100,
            110,
        );
        assert_eq!(at(through_helper.clone()), None);
        through_helper.via = Some("helper".into());
        through_helper.sites = vec![Location {
            path: "/p/App.tsx".into(),
            start_byte: 42,
            end_byte: 58,
        }];
        assert_eq!(at(through_helper), Some(Span::new(42, 58)));
        // A leaf scope whose callback is unresolved stands at the leaf call and answers for no site inside it.
        assert_eq!(
            at(leaf(
                crate::LeafOwnerOperationKind::UnresolvedCallback,
                "/p/App.tsx",
                40,
                60
            )),
            None
        );
        assert_eq!(
            delegate_row(Delegate::LeafOwnerForbiddenCall, &[], "/p/App.tsx", call),
            None
        );
    }

    #[test]
    fn a_hand_off_has_an_id_in_the_form_of_the_proof_log() {
        let delegate = Delegate::LeafOwnerForbiddenCall;
        let parts = delegate.id().split('.').collect::<Vec<_>>();
        assert!(
            parts.len() >= 2 && parts.iter().all(|part| !part.is_empty()),
            "{delegate:?}"
        );
    }

    #[test]
    fn a_removed_row_is_named_at_its_own_span() {
        let mut ends = SiteEnds::default();
        let location = Span::new(4, 11).location(Arc::<str>::from("/p/App.tsx"));
        ends.removed(
            &location,
            SiteSilence::ApplyNeverRuns,
            PremiseKind::ApplyNeverRuns,
            RemovedBy::Census,
        );
        assert_eq!(ends.silences(), vec![SiteSilence::ApplyNeverRuns]);
        assert!(ends.named().contains_key(&("/p/App.tsx", 4, 11)));
    }

    /// Every silence names the codes it is a proof for, except the ones that remove a row, which answer what the removed row answered. One arm per
    /// variant in `codes()`: a silence added without its codes is a compile error.
    #[test]
    fn every_silence_names_its_codes() {
        for silence in SiteSilence::ALL {
            assert_eq!(
                silence.codes().is_empty(),
                silence.removes_a_row(),
                "{silence:?}: a silence names codes, or the row it removed, never both and never neither"
            );
        }
    }

    fn end_of(silence: SiteSilence, removed: Option<RemovedBy>) -> SiteEnd {
        SiteEnd {
            path: "/p/App.tsx".into(),
            site: Span::new(0, 5),
            silence,
            premises: Vec::new(),
            removed,
        }
    }

    #[test]
    fn a_silence_answers_the_codes_of_its_own_table_and_no_other() {
        use solid_dialect::Dialect;
        let policy = |code: SiteCode| solid_dialect::Solid2.code_policy(code.runtime_code());
        // A read silence ends a read, never the write or the optimistic write at the same span.
        let read = end_of(SiteSilence::MemberPropsStatic, None);
        assert!(read.answers(policy(SiteCode::StrictRead).as_ref(), SiteCode::StrictRead));
        assert!(!read.answers(
            policy(SiteCode::ReactiveWrite).as_ref(),
            SiteCode::ReactiveWrite
        ));
        // A call outside every function has no owner: the owned-scope codes end, the optimistic write before the load does not.
        let module = end_of(SiteSilence::UnreachedWriteCannotRun, None);
        for code in [SiteCode::ReactiveWrite, SiteCode::ActionCall] {
            assert!(module.answers(policy(code).as_ref(), code), "{code:?}");
        }
        assert!(!module.answers(
            policy(SiteCode::OptimisticWrite).as_ref(),
            SiteCode::OptimisticWrite
        ));
        // The unguarded optimistic setter ends the owned-scope write only.
        let unguarded = end_of(SiteSilence::SetterOptimisticSignalUnguarded, None);
        assert!(unguarded.answers(
            policy(SiteCode::ReactiveWrite).as_ref(),
            SiteCode::ReactiveWrite
        ));
        assert!(!unguarded.answers(
            policy(SiteCode::OptimisticWrite).as_ref(),
            SiteCode::OptimisticWrite
        ));
    }

    #[test]
    fn a_removed_row_ends_the_codes_it_answered() {
        use solid_dialect::Dialect;
        let policy = |code: SiteCode| solid_dialect::Solid2.code_policy(code.runtime_code());
        let removed = |by| end_of(SiteSilence::ApplyNeverRuns, Some(by));
        // A read removed from an apply that never runs ends the read; an SC9012 every code that names a line; a style finding or an async read's row none.
        let read = removed(RemovedBy::of(&proof_universe::READ_ROW).unwrap());
        assert!(read.answers(policy(SiteCode::StrictRead).as_ref(), SiteCode::StrictRead));
        assert!(!read.answers(
            policy(SiteCode::ReactiveWrite).as_ref(),
            SiteCode::ReactiveWrite
        ));
        let census = removed(RemovedBy::Census);
        for code in SiteCode::ALL {
            assert!(census.answers(policy(code).as_ref(), code), "{code:?}");
        }
        let style = removed(RemovedBy::Rule {
            name: "prefer-show".into(),
            uncertain: false,
        });
        let pending = removed(RemovedBy::of(&proof_universe::ASYNC_READ_ROW).unwrap());
        for code in SiteCode::ALL {
            assert!(!style.answers(policy(code).as_ref(), code), "{code:?}");
            assert!(!pending.answers(policy(code).as_ref(), code), "{code:?}");
        }
    }

    #[test]
    fn a_site_gets_the_uncertifiable_row_of_its_own_code() {
        let source = "export const a = 1;\n";
        let ast = solid_facts::ast::extract("/p/App.tsx", source).unwrap();
        let file = FileFacts {
            generation: solid_facts::core::Generation::new(1).unwrap(),
            path: solid_facts::core::SourcePath::new("/p/App.tsx").unwrap(),
            source_hash: ast.source.hash.clone(),
            source: source.into(),
            compiler: Arc::new(solid_facts::compiler::ExecutionMap {
                compiler_facts_protocol: solid_facts::compiler::COMPILER_FACTS_PROTOCOL,
                source_hash: ast.source.hash.clone(),
                semantic_model: Default::default(),
                tracked_regions: vec![],
                untracked_regions: vec![],
                discarded_regions: vec![],
                ownership_regions: vec![],
                callback_roles: vec![],
                jsx_operations: vec![],
            }),
            ast: Arc::new(ast),
        };
        let site = Site {
            table: "write",
            source: "setter",
            via: "call",
            at: Span::new(13, 14),
            alternates: Vec::new(),
            codes: &[SiteCode::ReactiveWrite, SiteCode::OptimisticWrite],
        };
        let write = unaccounted_row(&file, &site, SiteCode::ReactiveWrite);
        assert_eq!(
            (
                write.id.as_str(),
                write.rule.as_str(),
                write.analysis_context.as_str()
            ),
            (
                "SC2001",
                "no-write-in-pure-computation",
                crate::blockers::WRITE_UNACCOUNTED
            )
        );
        let optimistic = unaccounted_row(&file, &site, SiteCode::OptimisticWrite);
        assert_eq!(
            (
                optimistic.id.as_str(),
                optimistic.rule.as_str(),
                optimistic.analysis_context.as_str()
            ),
            (
                "SC2009",
                "optimistic-write-before-load",
                crate::blockers::OPTIMISTIC_WRITE_UNACCOUNTED
            )
        );
        // Never a violation: an obligation, at the site, that yields to any row that stands.
        assert!(write.uncertain && optimistic.uncertain);
        assert_eq!(write.location, optimistic.location);
    }
}
