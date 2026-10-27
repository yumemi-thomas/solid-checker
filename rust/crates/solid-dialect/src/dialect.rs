//! The `Dialect` trait: every question the engine asks of a Solid vocabulary.

use super::*;

/// The owner-requirement partition every dialect so far agrees on: the three
/// effect constructors register a computation on the owner, and the two
/// cleanup registrars are each their own role. A dialect that adds a role
/// (Solid 2's `useContext`, ADR 0352) matches its own and delegates the rest
/// here, so the shared list is written once.
pub(crate) fn shared_owner_requirement_role(primitive: Primitive) -> Option<OwnerRequirementRole> {
    match primitive {
        Primitive::CreateEffect
        | Primitive::CreateRenderEffect
        | Primitive::CreateTrackedEffect => Some(OwnerRequirementRole::Effect),
        Primitive::OnCleanup => Some(OwnerRequirementRole::Cleanup),
        Primitive::OnSettled => Some(OwnerRequirementRole::SettledCleanup),
        _ => None,
    }
}

/// One Solid language version's vocabulary.
///
/// Implementors are stateless; a dialect is a set of tables, and every method
/// is a lookup.
pub trait Dialect: Sync {
    /// Exact candidates only. Missing vocabulary supplies no package constant.
    fn host_boolean_exports(&self) -> &'static [HostBooleanExport] {
        &[]
    }

    /// Classify an exact canonical declaration file and name-node byte span.
    /// No answer is not a veto: this is a conditional premise, not a census.
    fn runtime_configuration_api(
        &self,
        declaration_path: &str,
        start: u64,
        end: u64,
    ) -> Option<RuntimeConfigurationApi> {
        let _ = (declaration_path, start, end);
        None
    }

    /// Harmless member reads after exact identification of the hydration host.
    /// The caller excludes writes and supplies only a static member name.
    fn hydration_host_member_read_is_harmless(&self, member: &str, truthiness_only: bool) -> bool {
        let _ = (member, truthiness_only);
        false
    }

    /// Which version this adapter speaks.
    fn version(&self) -> Version;

    /// The packages whose installed releases decide this vocabulary's
    /// release-dependent answers, and where each resolves from. Shared code
    /// resolves them and hands the result to
    /// [`Dialect::review_installation`]; it never learns what each one owns.
    ///
    /// The default is none: a dialect that names no owner has only the
    /// `solid-js` detection read, and its review says so.
    fn release_owners(&self) -> &'static [ReleaseOwner] {
        &[]
    }

    /// ADR 0324: whether the router release installed as `@solidjs/router`
    /// at exactly `version` was audited to hand out route parameters as URL
    /// path-segment strings (or `undefined` for an absent optional one). A
    /// release nobody read answers `false`: the premise is not applied to it.
    /// The default is `false`, a dialect with no router audit.
    fn router_params_are_url_strings(&self, version: &str) -> bool {
        let _ = version;
        false
    }

    /// ADR 0324: the digest of the installed bytes of the audited
    /// `@solidjs/router` release `version`, as `scripts/router-digest.mjs`
    /// computes it, or `None` for a release nobody read. The premise applies
    /// only to an install whose bytes hash to exactly this.
    fn audited_router_digest(&self, version: &str) -> Option<&'static str> {
        let _ = version;
        None
    }

    /// The `@solidjs/router` releases audited for route parameters
    /// ([`Dialect::router_params_are_url_strings`]), for the notice that names
    /// them.
    fn audited_router_releases(&self) -> &'static [&'static str] {
        &[]
    }

    /// ADR 0334: the audited `@tanstack/solid-router` release `version`, whose
    /// installed code only renders the `component`, `pendingComponent`,
    /// `errorComponent` and `notFoundComponent` values of a route's options
    /// (or reads a member such as `preload` of them) and never calls one, or
    /// `None` for a release nobody read. The default is `None`.
    fn audited_route_components(&self, version: &str) -> Option<AuditedRouteRelease> {
        let _ = version;
        None
    }

    /// ADR 0335: the digest of the installed bytes of `@solidjs/router`
    /// release `version` (`scripts/router-digest.mjs`) when that release was
    /// audited to render the `component` of each route definition handed to
    /// `createRouter` with `createComponent` and never call it, or `None`.
    fn audited_route_config_digest(&self, version: &str) -> Option<&'static str> {
        let _ = version;
        None
    }

    /// The installation this vocabulary was audited on, as `(package,
    /// version)` in [`Dialect::release_owners`] order: what a user pins to
    /// certify. Empty for a dialect audited on none.
    fn audited_installation(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }

    /// How this vocabulary stands against one project's installation: one
    /// [`InstalledRelease`] per [`Dialect::release_owners`] entry, in order,
    /// each exactly as resolved.
    ///
    /// Asked of the language's own vocabulary, never of a variant, so a
    /// variant never judges the installations that select it.
    ///
    /// The default analyzes under the language's vocabulary with one gap: a
    /// dialect that has not said which releases it was read on has not been
    /// read on any, and the fail-visible answer is a notice rather than
    /// silence.
    fn review_installation(&self, installed: &[InstalledRelease<'_>]) -> InstallationReview {
        let _ = installed;
        InstallationReview::Analyzed {
            vocabulary: None,
            gaps: vec![InstallationGap {
                gap: "this vocabulary names no release it was read on".into(),
                review: None,
                scope: None,
            }],
        }
    }

    /// Which release variant of its language this vocabulary is: `None` for
    /// the language's own vocabulary, and otherwise a stable key naming how
    /// its answers differ from it. The key is part of every identity a result
    /// is cached under, so two variants must never share one, and one key
    /// must name the same answers in every build: spell it from the answers,
    /// not from which release happens to be audited.
    fn variant_key(&self) -> Option<&'static str> {
        None
    }

    /// Every release variant [`Dialect::review_installation`] can answer
    /// with, other than the language's own vocabulary: an enumeration for
    /// tests and audits. A variant is found again by its key with
    /// [`Dialect::variant_by_key`], which builds nothing.
    fn variants(&self) -> &'static [&'static dyn Dialect] {
        &[]
    }

    /// The variant whose [`Dialect::variant_key`] is `key`, decoded from the
    /// key itself rather than looked up among [`Dialect::variants`], so
    /// resolving an id builds no table of every variant. `None` for a key no
    /// variant of this language spells.
    fn variant_by_key(&self, key: &str) -> Option<&'static dyn Dialect> {
        let _ = key;
        None
    }

    /// The vocabulary a project is analyzed under when no installation of
    /// the language resolves at all, so nothing was reviewed: the language's
    /// own vocabulary or one of its [`Dialect::variants`]. Asked of the
    /// language's own vocabulary.
    ///
    /// The default is `None`, the language's own vocabulary.
    fn defaulted_vocabulary(&self) -> Option<&'static dyn Dialect> {
        None
    }

    /// Whether a binding spelling is a dialect convention that makes
    /// component identity possible but does not prove it.
    ///
    /// This is intentionally an uncertainty signal. A name cannot establish
    /// runtime invocation through JSX, but Solid 1's uppercase convention is
    /// enough to prevent the analyzer from certifying an ambiguous function
    /// as an ordinary helper.
    fn component_name_may_be_component(&self, name: &str) -> bool {
        let _ = name;
        false
    }

    /// Whether a direct JSX return is sufficient component evidence.
    ///
    /// Solid 2 treats JSX-producing functions as components. Solid 1 requires
    /// a JSX call site or an exact component type instead; upstream's
    /// binding-name shortcut is not semantic proof.
    fn direct_jsx_return_is_component(&self) -> bool {
        false
    }

    /// The modules whose exports this dialect owns.
    ///
    /// 1.x splits across subpaths (`solid-js/store`, `solid-js/web`); 2.0 moves
    /// store APIs into core and the DOM package to `@solidjs/web`.
    fn modules(&self) -> &'static [&'static str];

    /// The packages whose own published bytes *define* this dialect's
    /// primitives, by exact package name.
    ///
    /// Not specifiers and not a superset of [`Dialect::modules`]: this names
    /// the archives in which a dialect-spelled declaration is the primitive's
    /// own implementation rather than a consumer's import of it. Primitive
    /// identity is granted by declaration *path*
    /// (`solid-reactive-ir`'s `declaration_path_is_solid_package`), which is
    /// correct for a consumer — TypeScript resolved the symbol into the
    /// package — and wrong inside these archives, where every local
    /// `createSignal`/`createTrackedEffect` becomes a "primitive call".
    ///
    /// Callback summaries consult this to withhold the domains that
    /// recognition would otherwise fabricate. No diagnostic reads it.
    fn primitive_defining_packages(&self) -> &'static [&'static str];

    /// The npm scopes whose packages belong to this dialect's ecosystem, each
    /// written with its trailing slash (`@solidjs/`).
    ///
    /// Wider than [`Dialect::primitive_defining_packages`] on purpose, and
    /// answering a different question: `@solidjs/router` defines no primitive
    /// and is still a package that exists because this dialect does. Used
    /// where the question is "does this manifest depend on Solid at all",
    /// never to grant a package semantics.
    fn ecosystem_scopes(&self) -> &'static [&'static str];

    /// Resolves an exported name to a primitive, or `None` when this dialect
    /// does not export it.
    fn primitive(&self, name: &str) -> Option<Primitive>;

    /// The exported name for a primitive in this dialect.
    fn name_of(&self, primitive: Primitive) -> Option<&'static str>;

    /// The primitive one concrete call of `primitive` denotes, once the
    /// options it passes are known.
    ///
    /// Some exports are two runtimes behind one name, selected by an option
    /// the runtime reads before anything else. Every other question on this
    /// trait is keyed by primitive and argument position, so a form whose
    /// execution, tracking and ownership all differ is a different primitive
    /// here rather than a flag threaded through each of them: the engine asks
    /// this once, where it resolves a call, and every later question answers
    /// for the form.
    ///
    /// `option(argument, key)` reads what the call's syntax proves about one
    /// boolean option key ([`OptionLiteral`]). An implementation must map
    /// [`OptionLiteral::Unknown`] to a form it states nothing for, never to
    /// the more common of the two runtimes. The default answers `primitive`
    /// for every call: most exports have one runtime.
    fn call_form(
        &self,
        primitive: Primitive,
        option: &dyn Fn(usize, &str) -> OptionLiteral,
    ) -> Primitive {
        let _ = option;
        primitive
    }

    /// The argument positions the legacy engine treats as the primitive's
    /// *primary* callback slots — the places a rule looks when it needs "the"
    /// effect or compute function of a call.
    ///
    /// Not a census of every argument that holds a callback: 2.0's
    /// `createEffect(compute, apply)` answers `[1]` here — the slot the
    /// missing-effect-function rule checks — while [`Dialect::callback_executions`]
    /// records that the tracked compute sits at 0 and the deferred apply at 1.
    /// A caller that wants to know where callbacks are and how they run must
    /// use [`Dialect::callback_executions`] (or its call-shape-aware form,
    /// [`Dialect::callback_execution_at`]), not this.
    ///
    /// The dialect split it exists for is still ADR 0001's: 1.x's
    /// `createEffect(fn, value?)` answers `[0]`, because index 1 there is a
    /// *seed value* and checking it would fire on every correct 1.x effect.
    fn callback_positions(&self, primitive: Primitive) -> &'static [usize];

    /// Whether this primitive explicitly clears tracking around its callback,
    /// or runs it later outside the creating computation's pass.
    ///
    /// Distinct from [`Dialect::callback_positions`], which says *where* a
    /// callback sits, not how it executes. The two are independent and the
    /// difference is load-bearing: `untrack(fn)` and `createMemo(fn)` both put
    /// a callback at index 0, and one clears tracking while the other tracks.
    /// Asking only about position would classify a memo's compute as deferred
    /// and stop reporting reads inside it.
    fn runs_callback_deferred(&self, primitive: Primitive) -> bool;

    /// Whether this callback suppresses the strict pending-accessor safeguard.
    /// Ordinary NotReadyError can still propagate as graph suspension. This
    /// says nothing about store proxy reads or subscription: their runtime
    /// paths can differ.
    fn callback_handles_pending_accessor_read(&self, _primitive: Primitive, _index: usize) -> bool {
        false
    }

    /// Whether this primitive clears tracking around a callback it nonetheless
    /// runs **before its own call returns**.
    ///
    /// [`Dialect::runs_callback_deferred`] answers one boolean for two
    /// independent questions — "the listener is cleared" and "it runs later" —
    /// because its attribution consumer, `callback_runs_outside_tracking`, asks
    /// only the first. A callback summary's `execution` word asks the second:
    /// it is a statement about *when* the export invokes a caller-supplied
    /// callback, and the execution-word vocabulary keeps `untrack`,
    /// `createRoot` and `runWithOwner` at `inline` while the listener-clearing
    /// fact travels separately.
    ///
    /// This is the "detached, not later" half, and it is **derived rather than
    /// enumerated** so the two answers cannot drift: exactly the members of
    /// [`Dialect::runs_callback_deferred`] whose own
    /// [`Dialect::callback_executions`] rows are all [`Execution::Inline`]. A
    /// primitive the dialect models no callback for answers `false` — absence
    /// of a row is not evidence of synchrony. `the_synchronous_clearing_set_*`
    /// pins the resulting set per dialect.
    /// ADR 0183: whether the callback at `argument` of a call with
    /// `argument_count` arguments runs during the call, synchronously and
    /// inline: the primitive [runs its callback synchronously](Self::runs_callback_synchronously)
    /// and this slot's row is [`Execution::Inline`].
    /// A callback slot's invocation trigger under the ordinary runtime
    /// configuration. Defaults only reuse the proven synchronous slot.
    fn browser_callback_trigger(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> Option<BrowserCallbackTrigger> {
        self.synchronous_callback_slot(primitive, argument, argument_count)
            .then_some(BrowserCallbackTrigger::DuringCall)
    }

    /// Function children are not invoked merely because a tag is constructed.
    fn browser_children_trigger(&self, _primitive: Primitive) -> Option<BrowserCallbackTrigger> {
        None
    }

    /// Exact reviewed object-member callback slots; no guessed member dispatch.
    fn browser_callback_member_trigger(
        &self,
        _primitive: Primitive,
        _argument: usize,
        _argument_count: usize,
        _member: &str,
    ) -> Option<BrowserCallbackTrigger> {
        None
    }

    /// Options slot whose exact `defer: true` withholds initial delivery of
    /// this callback. Future dependency demand needs a separate witness.
    fn browser_callback_defer_options(
        &self,
        primitive: Primitive,
        argument: usize,
    ) -> Option<usize> {
        self.effect_apply_defer_options(primitive, argument)
    }

    /// Options slot that can withhold an effect apply's initial execution.
    /// This is an execution premise for every consumer, not just the browser
    /// host graph. Later delivery requires a changing compute dependency.
    fn effect_apply_defer_options(&self, _primitive: Primitive, _argument: usize) -> Option<usize> {
        None
    }

    /// A dialect control-flow condition can prove children never selected.
    fn browser_children_dead(
        &self,
        _primitive: Primitive,
        _when_false: bool,
        _each_empty: bool,
        _count_zero: bool,
    ) -> bool {
        false
    }

    fn synchronous_callback_slot(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> bool {
        self.runs_callback_synchronously(primitive)
            && self
                .callback_semantics_at(primitive, argument, argument_count)
                .execution
                == Some(Execution::Inline)
    }

    fn runs_callback_synchronously(&self, primitive: Primitive) -> bool {
        let rows = self.callback_executions(primitive);
        self.runs_callback_deferred(primitive)
            && !rows.is_empty()
            && rows
                .iter()
                .all(|(_, execution)| *execution == Execution::Inline)
    }

    /// When this primitive's [`Execution::Tracked`] callback at `argument` runs,
    /// relative to the primitive's own call returning.
    ///
    /// The second half of the schedule split described above
    /// ([`Dialect::runs_callback_synchronously`] is the first), and the fact
    /// that decides what a *clearing wrapper nested inside a tracked one*
    /// composes to in a callback summary. Reading `Tracked` as "runs
    /// later" is false for most of 1.x's own tracked primitives: `createMemo`
    /// and `createRenderEffect` run the computation during the call, and only
    /// `createEffect` queues it.
    ///
    /// **`None` is a refusal, not a default.** It means this dialect has
    /// established no schedule for that callback — because the audited runtime
    /// was not read for it, because the primitive never invokes the argument at
    /// all (1.x `createSignal(fn)` stores it), or because the shape resisted
    /// measurement. A callback summary leaves that exact callback leaf open rather
    /// than guessing, so a missing answer costs precision and never
    /// correctness. It is the direction to fail in, and *not* a licence to
    /// leave a member out because its name looks like a neighbour's: the two
    /// dialects disagree on `createEffect`, so the neighbour argument is
    /// exactly the one that produces a wrong claim. Every implemented answer
    /// cites the audited runtime source it was read from, and
    /// `the_tracked_callback_schedule_*` pins the resulting sets per dialect.
    fn tracked_callback_timing(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> Option<TrackedCallbackTiming> {
        let _ = (primitive, argument, argument_count);
        None
    }

    /// Whether this dialect's children-forbidden leaf callbacks
    /// ([`CallbackOwner::Leaf`]) are legal write/action regions.
    ///
    /// The `@solidjs/signals@2.0.0-rc.0` write guard exempts them: the
    /// setter, `refresh`, and action guards all test
    /// `owner && !(owner._config & CONFIG_CHILDREN_FORBIDDEN)` (dev bundle
    /// `dev.js:3154-3172`, `:3316-3331`, `:4312-4400`), with the runtime's
    /// own comment "leaf imperative scopes (tracked effects, onSettled) stay
    /// legal". Probed on the published rc.0 bundle: `setSignal`, `refresh`,
    /// and an action invocation inside `createTrackedEffect` and an
    /// owner-backed `onSettled` all succeed. 1.x has no such scopes, so the
    /// default is `false`.
    fn leaf_scopes_allow_writes(&self) -> bool {
        false
    }

    /// Whether this primitive's inline callback keeps the caller's *owner*
    /// context while clearing only the tracking listener — so a reactive
    /// write (or refresh/action invocation) inside it is exactly as legal or
    /// illegal as at the call site itself.
    ///
    /// The rc.0 write guard keys on the ambient owner, not on tracking:
    /// `untrack` swaps `tracking` and leaves `context` untouched
    /// (`dev.js:2928-2942`), so a write inside `untrack(...)` within a memo,
    /// component body, or effect compute throws
    /// `REACTIVE_WRITE_IN_OWNED_SCOPE`, while the same `untrack` write in an
    /// event handler succeeds (both probed). Read semantics are not this
    /// method's question — `untrack` stays an untracked-read scope either
    /// way.
    fn callback_preserves_owner_write_context(&self, primitive: Primitive) -> bool {
        let _ = primitive;
        false
    }

    /// Whether the callback at `argument` runs only while the runtime disposes
    /// an owner: a cleanup registrar's callback (`onCleanup`), which the
    /// engine's `runDisposal` calls with its disposal depth raised. A callback
    /// that also runs outside a disposal (an effect's returned cleanup, which a
    /// rerun calls outside `runDisposal`'s bracket) is not one. 1.x has no such
    /// question, so the default is `false`.
    fn callback_runs_only_in_disposal(&self, primitive: Primitive, argument: usize) -> bool {
        let _ = (primitive, argument);
        false
    }

    /// What the call returns for the disposal callback at `argument`
    /// ([`Dialect::callback_runs_only_in_disposal`]): the callback itself, so
    /// code that calls the result runs the callback at that call, outside the
    /// disposal; something else; or unknown, for a release nobody read. Asked
    /// only of a disposal callback. The default is the fail-closed `Unknown`.
    fn disposal_callback_returned(
        &self,
        primitive: Primitive,
        argument: usize,
    ) -> DisposalCallbackReturn {
        let _ = (primitive, argument);
        DisposalCallbackReturn::Unknown
    }

    /// Whether the cleanup the callback at `argument` of `primitive` returns
    /// (an effect's apply, not its compute, whose value feeds the apply) is called
    /// outside a disposal: a rerun of the effect calls it without the disposal
    /// bracket `callback_runs_only_in_disposal` names. Only a proven `true`
    /// places a write in such a cleanup outside a disposal; the default is
    /// `false`, the fail-closed answer.
    fn returned_cleanup_runs_outside_disposal(
        &self,
        primitive: Primitive,
        argument: usize,
    ) -> bool {
        let _ = (primitive, argument);
        false
    }

    /// What the dev owned-scope write guards (signal and store setters,
    /// `refresh`) do while the runtime disposes an owner
    /// ([`WriteGuardInDisposal`]): throw (a release before the `!disposalDepth`
    /// test), stand down, or unknown, for a release nobody read. An action
    /// call's guard is no such guard and never stands down. The default is the
    /// fail-closed `Unknown`: a dialect that does not read the release leaves a
    /// write that only a disposal decides uncertifiable, not legal and not
    /// reported.
    fn write_guard_in_disposal(&self) -> WriteGuardInDisposal {
        WriteGuardInDisposal::Unknown
    }

    /// Whether the callback at `argument`, whenever it runs, runs directly
    /// under a **root** owner the call creates for it, so that the ambient
    /// owner a write guard reads there is that root.
    ///
    /// *When* it runs is [`Dialect::callback_executions`]'s answer, not this
    /// one's: rc.17's `hydrate` may run `code` after it returns, under the
    /// root, and states no execution. A root is a children-capable owner, so a
    /// write guard that exempts only children-forbidden leaves rejects a write
    /// there. Nothing about the callback's *reads* follows from this: a root
    /// body is untracked. The default claims nothing, which leaves such a
    /// write to the ordinary callback classification.
    fn callback_runs_in_created_root(&self, primitive: Primitive, argument: usize) -> bool {
        let _ = (primitive, argument);
        false
    }

    /// Whether the callback at `argument` runs during the call inline, and
    /// this vocabulary has not read that the call creates a root for it. Such
    /// a write's legality depends on a fact nobody measured, so it is unplaced
    /// (uncertifiable), never legal. A callback that is a root on this release
    /// answers [`Dialect::callback_runs_in_created_root`] instead, and the
    /// default claims nothing.
    fn callback_root_is_unaudited(&self, primitive: Primitive, argument: usize) -> bool {
        let _ = (primitive, argument);
        false
    }

    /// Whether the callback at `argument` runs synchronously at the call, in the
    /// strict-read window the call itself is in, while clearing the listener.
    ///
    /// Two independent facts about a callback that an execution role conflates.
    /// [`Dialect::runs_callback_deferred`] says reads in it subscribe nothing of
    /// the caller's. This says whether a read in it still warns: a strict-read
    /// window belongs to the *call position* (the component body's dev
    /// `untrack(fn, label)`), and a primitive that swaps only the owner and the
    /// listener leaves it open, so the callback reports exactly where the call
    /// does and nowhere else. The default claims nothing.
    fn callback_inherits_calling_window(&self, primitive: Primitive, argument: usize) -> bool {
        let _ = (primitive, argument);
        false
    }

    /// Whether a store setter called with a root as the ambient owner is legal,
    /// when a signal setter there is not.
    ///
    /// `@solidjs/signals` 2.0.0-rc.1 through rc.8 exempt a root from the store
    /// setter's dev guard and not from `setSignal`'s; rc.9 exempts it from
    /// neither. So the answer belongs to the release that implements the guard
    /// ([`Dialect::review_installation`]). The default is `true`, the
    /// answer that claims no write: for a release nobody read, reporting a
    /// store write the runtime accepts would be a false violation, while
    /// silence is only a miss.
    fn store_setter_guard_exempts_roots(&self) -> bool {
        true
    }

    /// Whether the setter `createOptimisticStore` returns meets the dev
    /// owned-scope write guard at all.
    ///
    /// `@solidjs/signals` 2.0.0-rc.0 routes an optimistic store's writes
    /// through the optimistic engine, which never reaches a guard, so its
    /// setter is legal in every owned scope; rc.1 onwards guard every store
    /// setter at its entry. `false` makes an owned-scope write through that
    /// setter legal wherever it is, and one through a store setter not proven
    /// to be `createStore`'s unclaimed, since it may be the optimistic one. The
    /// default is `true`: the setter is judged as every other store setter is.
    fn optimistic_store_setter_guarded(&self) -> bool {
        true
    }

    /// Whether the ambient owner directly in a component body is a **root**,
    /// in the build whose write guards the rules model, so that a write there
    /// meets the guard exactly as one directly in a created root's body
    /// ([`Dialect::callback_runs_in_created_root`]) does.
    ///
    /// Only the root-exemption answers read this: an operation whose guard
    /// rejects every children-capable owner is reported in a component body
    /// either way. The default is `false`, which keeps a component body the
    /// ordinary owned scope and exempts nothing.
    fn component_body_runs_under_root(&self) -> bool {
        false
    }

    /// Whether this primitive's [`CallbackOwner::Leaf`] callback only
    /// materializes as a leaf owner when the call executes under a live,
    /// children-capable owner.
    ///
    /// rc.0's `onSettled` called out-of-band — from an event handler, with no
    /// owner, or inside another leaf scope — enqueues its callback as a plain
    /// function (`dev.js:4855-4893`): `onCleanup` inside it warns
    /// `NO_OWNER_CLEANUP` instead of throwing, primitives attach nowhere but
    /// do not throw, and `flush()` is a no-op. Only an owner-backed call
    /// becomes `createTrackedEffect(() => untrack(cb))`, the leaf owner the
    /// leaf-scope rules describe. `createTrackedEffect` itself is a leaf
    /// unconditionally, so the default is `false`.
    fn leaf_owner_requires_owned_call_site(&self, primitive: Primitive) -> bool {
        let _ = primitive;
        false
    }

    /// The owner a callback sees when a reviewed fresh-stack host scheduler
    /// runs it: `setTimeout`, `queueMicrotask`, `Promise.then`, an observer
    /// callback and the rest of the analyzer's `FRESH_STACK_SCHEDULERS`, which
    /// invoke the callback from a task or microtask queue on an otherwise
    /// empty execution-context stack. The owner analysis asks it again for
    /// code after an `await`, which resumes from a promise continuation on the
    /// same empty stack.
    ///
    /// The host half (the stack is empty) is the analyzer's reviewed fact; this
    /// is the dialect half: what that empty stack means for the owner. A
    /// runtime whose owner is a synchronous dynamic scope has none current
    /// there, whatever owner the scheduling call ran under. A runtime that
    /// carried its owner across tasks (an `AsyncContext`-style variable) would
    /// answer [`CallbackOwner::Inherits`] instead, which is why this is asked
    /// rather than assumed.
    ///
    /// **`None` is a refusal, not a default.** It gives such a callback no owner
    /// edge at all, so an owner requirement inside it keeps whatever lexical
    /// answer a region around it gives -- the answer every dialect had before
    /// this question existed.
    fn fresh_stack_callback_owner(&self) -> Option<CallbackOwner> {
        None
    }

    /// Whether this dialect's store type makes the **root record's own
    /// properties** `readonly`, so a direct write to one is already a
    /// TypeScript error and this checker must not report it as well.
    ///
    /// 2.0 returns a shallowly `Readonly` proxy from `createStore`, so
    /// `state.count = 1` and `state.count++` are both TS2540 ("Cannot assign to
    /// 'count' because it is a read-only property") against
    /// `@solidjs/signals@2.0.0-rc.0`. The readonly-ness stops at the top level:
    /// `state.user.name = "b"` type-checks, and so does every write through a
    /// props object, so those stay this checker's to report.
    ///
    /// 1.x is the opposite and the default is `false`: its `createStore` returns
    /// a mutable store type, and the same four writes produce **no** diagnostic
    /// at all (verified against `solid-js@1.9.14`). The 1.x rule is therefore
    /// fully independent -- which is exactly why this is asked of the dialect
    /// instead of assumed from the 2.0 answer.
    ///
    /// It is also the first answer that differs between two prereleases of
    /// one major: `@solidjs/signals` declares `Store<T> = T` from `2.0.0-rc.7`,
    /// so TS2540 is gone there while the runtime still drops the write. A
    /// vocabulary therefore answers this for the installation it was selected
    /// for ([`Dialect::review_installation`]), from the release of the package
    /// that declares the type, never for its major alone.
    fn store_root_properties_are_readonly(&self) -> bool {
        false
    }

    /// Whether a store's own setter callback write-enables the *original*
    /// store proxy for the duration of the callback.
    ///
    /// 2.0 puts the store into its Writing set while the draft callback
    /// runs, so `setStore(draft => { store.value = 7 })` commits through the
    /// original proxy exactly like a draft write (probed on rc.0; a write
    /// through *another* store's proxy in that callback is still silently
    /// dropped). 1.x setters take path arguments or pure updaters and never
    /// unlock the proxy, so the default is `false`.
    fn store_setter_callback_enables_proxy_writes(&self) -> bool {
        false
    }

    /// Which owner each of this primitive's callback arguments runs under.
    ///
    /// Empty means the dialect does not model the primitive's ownership, which
    /// is not the same as "it creates no owner" — a caller must treat an
    /// unlisted primitive as unknown rather than as [`CallbackOwner::None`].
    ///
    /// Indices are argument positions and need not match
    /// [`Dialect::callback_positions`]: 2.0's `createEffect(compute, apply)`
    /// tracks at index 1 but owns at index 0, because the apply phase runs
    /// after the compute's owner is established.
    fn callback_owners(&self, primitive: Primitive) -> &'static [(usize, CallbackOwner)] {
        let _ = primitive;
        &[]
    }

    /// The owner role of one callback argument in one concrete call.
    ///
    /// Like [`Dialect::callback_execution_at`], this defaults to the reviewed
    /// flat table and admits overload-specific overrides. Callers with an AST
    /// call must use this form so a value/source overload is never assigned a
    /// callback owner merely because another overload uses that slot.
    fn callback_owner_at(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> Option<CallbackOwner> {
        let _ = argument_count;
        self.callback_owners(primitive)
            .iter()
            .find(|(index, _)| *index == argument)
            .map(|(_, owner)| *owner)
    }

    /// The boundary role a JSX tag opens, if any.
    fn boundary_kind(&self, tag: &str) -> Option<Boundary>;

    /// This dialect's tag for a boundary role -- the inverse of
    /// [`Dialect::boundary_kind`].
    ///
    /// A diagnostic that tells the reader to wrap something in a boundary has
    /// to name one, and the name is the part that differs: 1.x says
    /// `Suspense`, 2.0 says `Loading`.
    fn boundary_name(&self, boundary: Boundary) -> &'static str;

    /// Whether this primitive may be created inside a leaf owner.
    fn cleanup_rule(&self, primitive: Primitive) -> CleanupRule;

    /// Whether a function passed at this primitive's callback position may
    /// return a cleanup.
    ///
    /// Narrower than every other callback question, and not derivable from
    /// them. [`Dialect::callback_positions`] answers for every
    /// callback-taking primitive, and a function returned from a memo's
    /// compute is its *value*; [`Dialect::runs_callback_deferred`] puts
    /// `untrack` on the same side as `onSettled`.
    ///
    /// The dialects disagree completely. Returning a cleanup is a 2.0 idea:
    /// 1.x's `EffectFunction<Prev, Next> = (v: Prev) => Next` threads the
    /// return value to the next run as `prev`, so
    /// `createEffect(prev => prev + 1, 0)` is idiomatic accumulation and
    /// nothing in 1.x reads a returned function as a cleanup.
    fn accepts_cleanup_return(&self, primitive: Primitive) -> bool;

    /// Whether this component renders its children through a callback the
    /// component itself invokes.
    ///
    /// A function written inside one is a callback, not a component, and its
    /// body runs per item or per branch rather than once. Both dialects have
    /// `For`, `Show`, `Match` and `Switch`; the fifth differs — `Repeat` in
    /// 2.0, `Index` in 1.x — which is the whole reason this is asked of the
    /// dialect rather than matched locally.
    ///
    /// Boundary tags are deliberately absent from both. They render children
    /// directly, not through a callback.
    fn renders_children_through_callback(&self, primitive: Primitive) -> bool;

    /// Whether `primitive` runs its function child as a callback only when the
    /// function declares parameters (`child.length > 0`); a function child of
    /// length 0 is returned as an ordinary child and inserted, so it runs in
    /// the insert's tracked effect. Default: no such gate.
    fn children_callback_requires_parameters(&self, primitive: Primitive) -> bool {
        let _ = primitive;
        false
    }

    /// Whether calling this primitive produces a reactive source — an
    /// accessor, a store, or a tuple containing one.
    ///
    /// Source *discovery* is where a rule's chain starts: a read the engine
    /// cannot trace to a source is not a read it can report on. A single-value
    /// `returns` summary answers this for single-value returns; what it cannot
    /// express is a tuple, so `createSignal` and friends have always been
    /// answered here instead.
    ///
    /// 1.x has five a single-value summary cannot reach — `createResource`
    /// returns `[accessor, { mutate, refetch }]` — and until this was a
    /// dialect question the engine used 2.0's list for both, so a read through
    /// a 1.x resource was traced to nothing and reported nowhere.
    fn creates_reactive_source(&self, primitive: Primitive) -> bool;

    /// Whether creating this primitive registers a directive-applied owner.
    fn creates_directive_owner(&self, primitive: Primitive) -> bool;

    /// Whether a call of `primitive` returns a props object that **carries the
    /// reactivity of the props objects it was given**.
    ///
    /// Not a source factory, which is why it is not a
    /// [`Dialect::creates_reactive_source`] row: the merge creates nothing. It
    /// hands back an object whose property reads reach through to its
    /// arguments, so the result is a props root exactly when one of the
    /// arguments already is — `mergeProps({ name: "Anonymous" }, props)` is
    /// reactive because `props` is, and `mergeProps({ a: 1 }, { b: 2 })` is a
    /// plain object whose destructuring loses nothing. The engine propagates
    /// the root only under that condition, so this row is a *permission to
    /// look at the arguments*, never a claim about the result on its own.
    ///
    /// Audited against the runtime, because the declaration cannot say it:
    /// 1.9.14's `mergeProps` declares `(...sources: T): MergeProps<T>`, while
    /// `dist/solid.js` returns a `$PROXY` when any source is a proxy or a
    /// function (memoised on the way in), and otherwise rebuilds the object
    /// *preserving each source's getters* — both shapes read through.
    ///
    /// This is a dialect question because the two dialects spell it
    /// differently and nothing else does: 1.x's `mergeProps` is 2.0's `merge`.
    /// The engine asked for the literal `"merge"` until this existed, so the
    /// whole propagation was silently 2.0-only — `fixtures/reactive-ir/
    /// eslint-plugin-corpus{,-v1}/props-extended-invalid.tsx` are the same
    /// ported upstream case in the two spellings, and only the 2.0 one was
    /// reported.
    fn merges_props_reactivity(&self, primitive: Primitive) -> bool;

    /// Whether `primitive` takes a props object and key lists and returns
    /// views over it, so every one of its arguments is a *value* — never a
    /// callback the caller hands over.
    ///
    /// The engine needs this to keep quiet: erased JavaScript types leave a
    /// key list's callability unknown, and without the row a split raises an
    /// unknown-callback obligation about arguments that cannot be callbacks.
    ///
    /// A dialect question for the same reason the merge above is: 1.x's
    /// `splitProps(props, ...keys)` is 2.0's `omit(props, ...keys)`
    /// (`@solidjs/signals`' `store/utils.d.ts`), and shared code used to name
    /// only 1.x's.
    ///
    /// "Every argument" has one exception a split can declare, and it
    /// outranks this answer: an argument
    /// [`Dialect::callback_runs_on_result_access`] names is a callback the
    /// runtime invokes, not a value.
    fn splits_props(&self, primitive: Primitive) -> bool;

    /// Whether the function at `argument` is invoked when the call's
    /// **returned object is read** — a property get, an `in` test, a key
    /// enumeration — in the reading code's tracking and ownership, rather than
    /// by the call itself.
    ///
    /// No [`Execution`] word describes that: the callback runs once per read,
    /// at whatever time and under whatever observer the reader has, and the
    /// engine does not follow reads of a returned view back to the call that
    /// made it. So the answer is a positive fact with a fail-closed consumer:
    /// the engine must treat such a callback as invoked at a time it cannot
    /// place — never as a value (which is what [`Dialect::splits_props`]
    /// would otherwise make it), and never as running where it is written.
    /// The engine reports such a callback uncertifiable unless it proves the
    /// body inert; it never reports a violation inside one.
    ///
    /// The case that needs it is rc.9's `omit(props, hidden)` predicate, so
    /// the answer is a release's, not a language's: a vocabulary answers it
    /// only for releases whose runtime has the form. The default is `false`.
    fn callback_runs_on_result_access(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> bool {
        let _ = (primitive, argument, argument_count);
        false
    }

    /// Whether the generator function at `argument` of a `primitive` call is
    /// an **action body**: the runtime drives it one step at a time, and each
    /// step -- the synchronous slice of the body that one `next()`/`throw()`
    /// of the generator runs -- executes with the runtime's action-step marker
    /// set, cleared again as the step returns.
    ///
    /// What counts as inside a step is then the language's, not the
    /// dialect's: a sync generator's every slice runs within its step, and an
    /// async generator's slice ends at its first suspension (an `await`, a
    /// `for await`, an async `yield*`), whose continuation runs from a
    /// microtask with no step on the stack. A plain `yield` hands the
    /// runtime the value and is resumed by the next step. Parameter
    /// initializers run when the generator object is created, before the first
    /// step. The engine proves positions against these; the dialect answers
    /// only which callback is stepped.
    ///
    /// Solid 2.0's `action(genFn)` is the case (`@solidjs/signals`'
    /// `action`, whose `step` brackets `it.next(v)` with
    /// `enterActionStep()`/`exitActionStep()` from `2.0.0-rc.8`). The default
    /// is `false`.
    fn callback_runs_as_action_steps(&self, primitive: Primitive, argument: usize) -> bool {
        let _ = (primitive, argument);
        false
    }

    /// The argument that a call of the export declared as `name` **renders**:
    /// the call invokes that argument as a component, synchronously, before
    /// it returns, on the caller's stack.
    ///
    /// That is what a compiled JSX tag does, and it is a different question
    /// from a primitive's callback slots. `name` is not a primitive: nothing
    /// about the call is reactive, and the vocabulary models no obligation for
    /// it. What the engine needs is one fact about control flow: a call to a
    /// project function passed there is an invocation of that function, the
    /// same edge the tag `<C/>` is, not a value escaping into a callee the
    /// graph cannot follow. The engine asks it only of a callee whose exact
    /// declaration sits in one of this dialect's packages, and it uses the
    /// answer only as a call edge (ADR 0136).
    ///
    /// Solid 2.0's `createComponent(Comp, props)` is the case. The default is
    /// `None`.
    fn renders_component_argument(&self, name: &str) -> Option<usize> {
        let _ = name;
        None
    }

    /// The argument of a call of `primitive` with exactly `arguments`
    /// arguments that is a **component loader whose `default` it renders**:
    /// the call returns a component that, when rendered, awaits the promise
    /// that loader returns and renders that module object's `default` member
    /// as a component, with the body's own render semantics. The call runs
    /// nothing synchronously, and the returned component's only other way to
    /// the module object is the one the engine must enumerate itself (2.0's
    /// `.preload()`, which hands the same promise to its caller).
    ///
    /// Like [`Dialect::renders_component_argument`], this is one fact about
    /// control flow and nothing about reactivity. The engine uses it only to
    /// withdraw an object-retention refusal from a project function whose one
    /// reference is that `default` member, and only after proving every other
    /// premise itself (ADR 0292). A call form that selects another member
    /// (2.0's `{ export: "Name" }` option) answers `None`, rather than letting
    /// the engine guess which member renders.
    ///
    /// Solid 2.0's one-argument `lazy(loader)` is the case. The default is
    /// `None`.
    fn renders_loaded_default_argument(
        &self,
        primitive: Primitive,
        arguments: usize,
    ) -> Option<usize> {
        let _ = (primitive, arguments);
        None
    }

    /// The part the export declared as `name` plays in this vocabulary's
    /// context mechanism (ADR 0153), or `None`.
    ///
    /// Three facts, and a census of a package-owned context rests on all
    /// three together: a [`ContextRole::Create`] call with no arguments makes a
    /// context whose only values are the ones its provider stores, a
    /// [`ContextRole::Read`] call hands back one of those values or throws,
    /// and a [`ContextRole::Render`] call runs its first argument with its
    /// second as props, which is how a compiled provider stores `props.value`.
    /// The engine asks it only of a callee whose exact declaration sits in an
    /// audited archive of this dialect. The default is `None`.
    fn context_role(&self, name: &str) -> Option<ContextRole> {
        let _ = name;
        None
    }

    /// The components that **render a prop's value** as a component: each
    /// `(export, prop)` pair names an export whose render, `<export
    /// prop={value}/>` or its compiled `renderer(export, { prop: value })`,
    /// invokes `value` as a component only inside computations that render
    /// creates, and hands it to nothing else that invokes it.
    ///
    /// Like [`Dialect::renders_component_argument`], this is one fact about
    /// control flow and nothing about reactivity. The engine asks it only of
    /// a component whose exact declaration sits in one of this dialect's
    /// packages, uses the answer only as a call-graph edge from the render to
    /// a project function whose value provably reaches that prop, and follows
    /// that value only through the holders
    /// [`Dialect::accessor_yields_only_its_compute`] vouches for (ADR 0138).
    ///
    /// Solid 2.0's deprecated `<Dynamic component={…}/>` is the case. The
    /// default is empty.
    fn component_prop_renderers(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }

    /// The components that render an **intrinsic element** when a prop is a
    /// string: each `(export, prop)` pair names an export whose render, given
    /// a string in `prop`, creates the element that string names and assigns
    /// it every *other* prop exactly as a JSX spread onto an intrinsic element
    /// does, so an un-namespaced `on…` prop becomes an event listener and is
    /// never called by the render.
    ///
    /// One fact about the string form only. A non-string value (a component,
    /// a promise, `undefined`) is not described here, and the engine asks only
    /// of a component whose exact declaration sits in one of this dialect's
    /// packages, after proving the prop's value is a string literal (ADR
    /// 0313). The default is empty.
    fn intrinsic_tag_renderers(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }

    /// Whether an intrinsic spread key installs an event listener. `None`
    /// means that runtime spelling rule has not been audited for this dialect.
    fn spread_key_is_event(&self, key: &str) -> Option<bool> {
        let _ = key;
        None
    }

    /// Whether an event dispatched on the global `document` object reaches no
    /// event handler this runtime installs from JSX (ADR 0435): its delegated
    /// handler starts at the event's target and looks for handler data on the
    /// elements of the path, and `document` carries none; undelegated handlers
    /// are listeners on their own elements, below the document. The default
    /// is `false`: an unaudited runtime may route events differently.
    fn document_dispatch_reaches_no_element_handler(&self) -> bool {
        false
    }

    /// Whether a call of `primitive` with exactly one argument, a compute
    /// function that takes no parameters, returns an accessor that is the
    /// only way the values the compute returns reach program code.
    ///
    /// The engine uses it only to follow a value through `const X =
    /// primitive(() => value)` read as `X()`, on the way to a rendering prop
    /// ([`Dialect::component_prop_renderers`], ADR 0138). It claims nothing
    /// about *which* values the accessor yields: a value the runtime
    /// substitutes (a hydrated one, say) is another value, and the flow the
    /// engine proves is only that this one goes nowhere else. The default is
    /// `false`.
    fn accessor_yields_only_its_compute(&self, primitive: Primitive) -> bool {
        let _ = primitive;
        false
    }

    /// Whether calling `primitive` while an action step
    /// ([`Dialect::callback_runs_as_action_steps`]) is on the stack **throws**
    /// in the development build of this installation.
    ///
    /// A release's answer, not a language's: `@solidjs/signals@2.0.0-rc.8`
    /// added the `FLUSH_IN_ACTION` throw to `flush`, and earlier releases
    /// drain as they do anywhere else. A vocabulary answers it only for the
    /// releases whose bytes have the throw. The default is `false`.
    fn throws_inside_action_step(&self, primitive: Primitive) -> bool {
        let _ = primitive;
        false
    }

    /// Whether the callback at `argument` of a `primitive` call is the **effect
    /// phase** of a user effect: the runtime queues it and runs it from inside
    /// the flush that drains effects, with the effect-callback marker set for
    /// exactly the duration of that call.
    ///
    /// A primitive that runs its callback at its own call, or only on a first
    /// run that is not queued, is not one: `createRenderEffect`'s first run is
    /// synchronous at creation, outside any flush, so a `flush` there still
    /// drains. The default is `false`.
    fn callback_runs_in_effect_phase(&self, primitive: Primitive, argument: usize) -> bool {
        let _ = (primitive, argument);
        false
    }

    /// Whether calling `primitive` synchronously from an effect phase
    /// ([`Dialect::callback_runs_in_effect_phase`]) is a **no-op**, one the
    /// development build usually reports (`FLUSH_IN_EFFECT_CALLBACK`, a
    /// warning).
    ///
    /// A release's answer: the flush that runs effects is already in progress,
    /// so a nested `flush` returns without draining in every client build.
    /// Only the no-op is the claim: the development build of
    /// `@solidjs/signals` from rc.1 warns through a global marker that a
    /// nested synchronous effect run lowers, so the warning is not
    /// guaranteed. An unread or unresolved release states nothing. The
    /// default is `false`.
    fn warns_inside_effect_phase(&self, primitive: Primitive) -> bool {
        let _ = primitive;
        false
    }

    /// Whether a call of `primitive` returns a **tuple** whose first slot
    /// carries the reactive value.
    ///
    /// The shape question, distinct from [`Dialect::returns_store`]'s kind
    /// question and from [`Dialect::reactive_result_slot`]'s per-slot role: a
    /// read traced through this call has to know it must go through slot 0
    /// rather than through the call's own value. `createMutable` (1.x) and
    /// `createProjection` (2.0) return the store *itself* and are therefore
    /// not rows here, however store-kinded they are.
    ///
    /// Each dialect's list is its own, and neither is the other's: 1.x's
    /// `createResource` returns `[accessor, { mutate, refetch }]` and does not
    /// exist in 2.0, while 2.0's `createOptimistic` returns
    /// `Signal<T> = [get, set]` and `createOptimisticStore` returns
    /// `[get: Store<T>, set: StoreSetter<T>]`, neither of which exists in 1.x.
    /// Shared code carried one hardcoded list that was neither dialect's.
    fn returns_reactive_tuple(&self, primitive: Primitive) -> bool;

    /// Whether a read of a pending async value inside this primitive's tracked
    /// computation is a *render* of that value, so that a missing async
    /// boundary above it is the read's own defect.
    ///
    /// Only a render effect reports the missing boundary: a memo, a user
    /// effect's compute or a derived signal that reads a pending value just
    /// propagates the pending state to whichever node finally consumes it, and
    /// the boundary question belongs to that consumer. Default `false`: a
    /// dialect that has not said which of its primitives render claims no
    /// boundary defect for a read in a computation.
    fn computation_read_is_render(&self, _primitive: Primitive) -> bool {
        false
    }

    /// Whether this primitive mounts a component tree at a DOM root, taking
    /// the root component (or a function returning it) as its first argument.
    ///
    /// The mount call is the end of every render chain: a component rendered
    /// there has no other parent, which is what lets an analysis prove the
    /// chain above a read contains no boundary instead of merely not seeing
    /// one. Default `false`: a dialect that names no mount primitive proves no
    /// chain complete.
    fn mounts_component_tree(&self, _primitive: Primitive) -> bool {
        false
    }

    /// Which parameters of a control-flow component's children callback are
    /// reactive accessors rather than plain values.
    ///
    /// Source discovery depends on this: a parameter the engine does not know
    /// is an accessor is not a reactive source, and a read of it is a read no
    /// rule can report on.
    ///
    /// The pair that makes this a dialect question is `<For>` and `<Index>`,
    /// which are exact mirrors of each other in 1.x —
    /// `For` hands out `(item, index: Accessor)` and `Index` hands out
    /// `(item: Accessor, index)`. The engine had `<For>`'s answer and no
    /// `<Index>` arm at all, so every `<Index>` item accessor in a 1.x project
    /// was invisible. 2.0 has no `<Index>`; it has `<Repeat>`, whose index is a
    /// plain number, and a three-way `keyed` prop on `<For>` that 1.x does not
    /// have.
    fn children_accessor_parameters(&self, primitive: Primitive, key: KeyForm) -> &'static [usize];

    /// Which parameters of one primitive callback are reactive accessors.
    ///
    /// This is the call-expression counterpart to
    /// [`Dialect::children_accessor_parameters`]. Solid 1.x `mapArray` hands
    /// its mapper `(item, index: Accessor<number>)`, while `indexArray` hands
    /// it `(item: Accessor<T>, index)`. Those sources come from runtime
    /// runtime semantics, not TypeScript return types, so source discovery must ask
    /// the dialect explicitly.
    fn callback_accessor_parameters(
        &self,
        primitive: Primitive,
        argument: usize,
    ) -> &'static [usize] {
        let _ = (primitive, argument);
        &[]
    }

    /// Whether what this primitive returns is a store rather than an accessor.
    ///
    /// The companion to [`Dialect::creates_reactive_source`], which says
    /// *whether* a call produces a source; this says *which kind*, and the
    /// engine branches on it to pick a `ReactiveSourceKind`.
    ///
    /// Core return classification is owned here, without a package overlay. For tuple positions use [`Dialect::reactive_result_slot`];
    /// this answer alone cannot distinguish a store from its setter.
    fn returns_store(&self, primitive: Primitive) -> bool;

    /// Whether an evaluated Get of this known string key on a runtime store
    /// can warn in this dialect's strict untracked read scope. This is not a
    /// claim about dependency tracking: protocol and symbol keys can differ.
    /// Dynamic/non-string keys must not be submitted as spelling guesses.
    /// An unaudited dialect fails closed.
    fn store_key_warns_strict_read(&self, _key: &str) -> bool {
        false
    }

    /// Whether an evaluated Get of this known string key on a runtime store
    /// may skip the strict-read warning that the dialect's store Get gives
    /// other string keys (ADR 0293). This is the refusal half of
    /// [`Self::store_key_warns_strict_read`], asked where a store Get was
    /// already a strict-read witness before that premise was audited: an
    /// answer of `true` withholds the violation. Non-string keys are never
    /// submitted; the caller decides them as undecided. An unaudited dialect
    /// fails closed and exempts every key.
    fn store_key_may_skip_strict_read(&self, _key: &str) -> bool {
        true
    }

    /// The callback argument whose exact returned value is the call's result.
    /// This is a runtime identity claim, separate from callback execution and
    /// ownership. Silence grants no passthrough to an unaudited dialect.
    fn returned_callback_value(&self, _primitive: Primitive) -> Option<usize> {
        None
    }

    /// The reactive role this dialect assigns to one slot of what `primitive`
    /// returns, or `None` where this dialect's audited vocabulary does not
    /// state it.
    ///
    /// The finer-grained companion to [`Dialect::creates_reactive_source`] and
    /// [`Dialect::returns_store`], which together say *whether* a call produces
    /// a source and *which kind* but carry no slot — so they cannot tell
    /// `createSignal()[0]` from `createSignal()[1]`, and a proof that read them
    /// for a slot would certify a setter as an accessor.
    ///
    /// `None` is not a negative claim. Every row is a per-version audited fact
    /// and stays absent until this dialect's owner has one, which is why a
    /// dialect's silence is never filled in from another dialect's row.
    fn reactive_result_slot(&self, primitive: Primitive, slot: ResultSlot) -> Option<ReactiveRole> {
        let _ = (primitive, slot);
        None
    }

    /// The exact `length` of the function a call of `primitive` returns in
    /// `slot`, when that slot is a getter this dialect builds and its
    /// audited source states the arity (ADR 0338). It lets a helper that
    /// tests `typeof p === "function"` or `p.length` be decided for a call
    /// that passes such a getter, without guessing from the declared
    /// type: `Accessor<T>` is `() => T` to the compiler and says nothing of
    /// a function's runtime arity.
    ///
    /// `None` states nothing: a slot that is not a getter, a getter the
    /// audit did not reach, or a dialect that has not audited it. It is
    /// never a claim that the length is non-zero.
    fn accessor_getter_length(&self, primitive: Primitive, slot: ResultSlot) -> Option<u32> {
        let _ = (primitive, slot);
        None
    }

    /// ADR 0183: whether a callback written at `argument` of a call of
    /// `primitive` with `argument_count` arguments runs during that call, at
    /// least once, as the compute of a computation the call creates -- under
    /// that computation's own, children-capable owner.
    ///
    /// The default `false` states nothing. A dialect answers `true` only where
    /// the runtime's eagerness is unconditional at that arity: an options
    /// argument that can defer the first run (`lazy`) must not be present.
    fn eager_owned_computation_slot(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> bool {
        let _ = (primitive, argument, argument_count);
        false
    }

    /// Which argument holds `primitive`'s options object.
    ///
    /// A separate index vocabulary from [`Dialect::callback_positions`], and it
    /// has to be: the two answers move independently between versions.
    /// `createMemo` is the case that forced this out of the engine — 2.0's is
    /// `(compute, options?)` and 1.x's is `(fn, value?, options?)`, so a
    /// checker reading index 1 for both reads 1.x's *seed value* as an options
    /// object. The dialect fixture pair had a recorded finding from exactly
    /// that.
    ///
    /// `None` where the dialect has no single answer. `createResource` takes
    /// its options at 1 or 2 depending on whether a source was supplied, the
    /// same ambiguity its callback position has, and a guess either way is
    /// worse than saying nothing.
    fn options_argument(&self, primitive: Primitive) -> Option<usize>;

    /// Whether this primitive's options include `sync`.
    ///
    /// An options slot alone is not evidence for a particular option key:
    /// Solid 1.x has options objects for memos, signals, and stores but no
    /// synchronous-node option. Keeping the key in the dialect prevents a
    /// 2.0-only diagnostic identity from reaching the 1.x rule catalog.
    fn supports_sync_option(&self, primitive: Primitive) -> bool {
        let _ = primitive;
        false
    }

    /// When each callback argument of `primitive` runs.
    ///
    /// Empty means the dialect models no callback for it — not a
    /// claim that no function can be passed.
    ///
    /// The 1.x/2.0 split this exists for: `createEffect` has one tracked
    /// callback in 1.x and a tracked compute plus a deferred apply in 2.0.
    /// Hardcoding the 2.0 pair described a read in 1.x's *seed value* as being
    /// in an "apply callback" that version does not have.
    fn callback_executions(&self, primitive: Primitive) -> &'static [(usize, Execution)];

    /// Whether every function-valued argument of `primitive` is wrapped in a
    /// memo of its own, so code in it runs inside that computation and not in
    /// the caller's body. Unknown is `false`.
    fn wraps_function_arguments_in_memo(&self, _primitive: Primitive) -> bool {
        false
    }

    /// Whether one callback argument describes work performed by a function
    /// returned from the primitive rather than by the primitive call itself.
    ///
    /// Call-site analysis must prove that returned function is invoked before
    /// treating these callbacks as reachable. This prevents a discarded lazy
    /// adapter from manufacturing reads, owners, or diagnostics.
    ///
    /// This is argument-sensitive because Solid 1.x `createSelector(source,
    /// comparator)` invokes `source` eagerly in its computation but cannot
    /// invoke `comparator` until the returned selector receives a key.
    fn callback_requires_return_invocation(&self, primitive: Primitive, argument: usize) -> bool {
        let _ = (primitive, argument);
        false
    }

    /// How a callback passed to the function returned by `primitive` runs.
    ///
    /// This is deliberately separate from [`Dialect::callback_execution_at`]:
    /// in Solid 1.x `createReaction(onInvalidate)` receives one deferred
    /// callback now, then returns a tracker that receives a different, tracked
    /// callback later. Flattening those two call signatures loses a runtime
    /// boundary that neither TypeScript overloads nor a first-order callback
    /// list can express.
    fn returned_callback_execution_at(
        &self,
        primitive: Primitive,
        result_slot: Option<usize>,
        argument: usize,
        argument_count: usize,
    ) -> Option<Execution> {
        let _ = (primitive, result_slot, argument, argument_count);
        None
    }

    /// The owner contract of a callback accepted by a function returned from
    /// `primitive`.
    ///
    /// `result_slot` preserves tuple identity. Solid 1.x `useTransition()`
    /// returns a pending accessor at slot 0 and a starter at slot 1; only the
    /// starter accepts a callback. TypeScript symbol identity plus the AST
    /// binding shape must prove that slot before an owner edge may exist.
    fn returned_callback_owner_at(
        &self,
        primitive: Primitive,
        result_slot: Option<usize>,
        argument: usize,
        argument_count: usize,
    ) -> Option<CallbackOwner> {
        let _ = (primitive, result_slot, argument, argument_count);
        None
    }

    /// The complete callback contract of one argument to a returned function.
    fn returned_callback_semantics_at(
        &self,
        primitive: Primitive,
        result_slot: Option<usize>,
        argument: usize,
        argument_count: usize,
    ) -> ReturnedCallbackSemantics {
        ReturnedCallbackSemantics {
            execution: self.returned_callback_execution_at(
                primitive,
                result_slot,
                argument,
                argument_count,
            ),
            owner: self.returned_callback_owner_at(
                primitive,
                result_slot,
                argument,
                argument_count,
            ),
        }
    }

    /// How one argument of one concrete call executes.
    ///
    /// This is the call-site form of [`Dialect::callback_executions`]. The
    /// table is the default; dialects override this method only for overloads
    /// whose roles depend on call shape and therefore cannot be represented by
    /// a flat parameter list. Solid 1.x `createResource` is
    /// the motivating case: `(fetcher)` defers argument 0, while
    /// `(source, fetcher)` tracks argument 0 and defers argument 1.
    fn callback_execution_at(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> Option<Execution> {
        let _ = argument_count;
        self.callback_executions(primitive)
            .iter()
            .find(|(index, _)| *index == argument)
            .map(|(_, execution)| *execution)
    }

    /// The **execution word** for a callback at this slot, which is a
    /// different question from [`Dialect::callback_execution_at`].
    ///
    /// That one answers attribution for the checker's own analysis: whose reads
    /// a callback subscribes. This one answers what a callback summary states
    /// about *observable scheduling relative to the exported call*. The two agree for most primitives and deliberately
    /// diverge for some — Solid 1.x's `createResource` fetcher is `Deferred`
    /// for attribution (it runs outside the source computation) and `Inline`
    /// for the word (both overloads invoke it before `createResource`
    /// returns), and 1.x's `on` returns an adapter whose callbacks run at the
    /// eventual invocation rather than during the creating call.
    ///
    /// `None` means this dialect states no execution word for the slot, which
    /// a summary must treat as "unknown" rather than as any of the three
    /// answers.
    ///
    /// This lived as a hardcoded `match` in `solid-reactive-ir`'s
    /// `interproc.rs` until ADR 0111. It belongs here: it is per-version
    /// behaviour, and shared code holding it meant a new dialect could not
    /// state a different answer.
    fn callback_execution_word_at(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> Option<Execution> {
        let _ = (primitive, argument, argument_count);
        None
    }

    /// Whether a function passed at `argument` is stored as a plain value the
    /// primitive never invokes.
    ///
    /// Positive knowledge only. Solid 1.x `createSignal(() => value)` keeps
    /// the function as the signal's value, so reads inside it are dormant —
    /// while the same source under 2.0 is a derived signal whose compute
    /// tracks them. Answering `false` means "unmodelled", never "invoked":
    /// a missing [`Dialect::callback_executions`] row (2.0 `children`,
    /// `onCleanup`) is not evidence in either direction, and engines must not
    /// treat that absence as proof of dormancy.
    fn stores_function_argument_as_value(&self, primitive: Primitive, argument: usize) -> bool {
        let _ = (primitive, argument);
        false
    }

    /// The complete callback contract for one concrete call argument.
    fn callback_semantics_at(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> CallbackSemantics {
        let execution = self.callback_execution_at(primitive, argument, argument_count);
        CallbackSemantics {
            execution,
            owner: self.callback_owner_at(primitive, argument, argument_count),
            tracks_reads: execution == Some(Execution::Tracked)
                && !self.runs_callback_deferred(primitive)
                && !self.callback_runs_untracked_without_window(primitive, argument),
            requires_return_invocation: self
                .callback_requires_return_invocation(primitive, argument),
            stores_as_value: self.stores_function_argument_as_value(primitive, argument),
            accessor_parameters: self.callback_accessor_parameters(primitive, argument),
        }
    }

    /// The argument that is this primitive's **apply** callback: the slot the
    /// runtime runs outside the tracked compute, with its strict-read window
    /// open, so a read there does not subscribe the effect and the phase
    /// deserves its own name.
    ///
    /// Outside the compute is not the same as after the call. 2.0's
    /// `createEffect` queues its apply, while `createRenderEffect` runs its
    /// first apply before the call returns; [`Dialect::callback_owners`] and
    /// [`Dialect::callback_execution_word_at`] carry that difference.
    ///
    /// Not the same question as "is this callback deferred". Several
    /// primitives defer a callback the caller supplies — an executor, a
    /// scheduler — and that callback keeps its enclosing label rather than
    /// becoming an apply phase. Only an effect constructor has an apply slot,
    /// and which slot it is, or whether the primitive has one at all, is a
    /// dialect's answer: 1.x's second `createEffect` argument is a seed value
    /// threaded to the next run as `prev`, so describing a read there as
    /// living in an "apply callback" would name a phase 1.x does not have.
    ///
    /// Defaults to `None`, which is the conservative answer: a dialect that
    /// says nothing claims no apply phase rather than inheriting 2.0's.
    fn apply_callback_argument(&self, primitive: Primitive) -> Option<usize> {
        let _ = primitive;
        None
    }

    /// The owner-requirement role this primitive's call carries, if any.
    ///
    /// The default is the partition every dialect so far has agreed on: the
    /// three effect constructors register a computation on the owner, and the
    /// two cleanup registrars are each their own role. It is a method rather
    /// than a `match` in shared code because *which* primitives register a
    /// computation is a dialect's answer — a dialect whose render effect did
    /// not register on the owner would say so here, not by having the owner
    /// pass learn its name.
    ///
    /// The owner passes used to match the sets directly, in two places, and
    /// the two drifted: one of them omitted `createRenderEffect`, so a render
    /// effect outside any owner leaked with nothing reported. One named answer
    /// is what stops that recurring quietly.
    fn owner_requirement_role(&self, primitive: Primitive) -> Option<OwnerRequirementRole> {
        shared_owner_requirement_role(primitive)
    }

    /// Whether an untracked read in this callback is a likely dependency bug.
    ///
    /// Most deliberately untracked callbacks are explicit imperative scopes:
    /// `untrack`, `onMount`, effect apply, and event-like callbacks. Solid 1.x
    /// resource fetchers are different. They look like computations but the
    /// runtime invokes them under `untrack`; dependencies must be declared in
    /// the source argument, so a reactive read in the fetcher is reportable.
    fn reports_untracked_reads_at(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> bool {
        let _ = (primitive, argument, argument_count);
        false
    }

    /// Whether the callback at `argument` may run in a strict-read window of
    /// its own, opened from an option the call may pass: a reactive read in it
    /// may then warn although the slot tracks its reads otherwise. A proof that
    /// a callback runs silently must refuse such a slot; this is not evidence
    /// that it warns.
    fn callback_may_open_strict_window(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> bool {
        let _ = (primitive, argument, argument_count);
        false
    }

    /// Whether the callback at `argument` runs with tracking off and opens no
    /// strict-read window of its own (ADR 0443): a reactive read in it
    /// subscribes nothing and, unless the call passes an option that opens one
    /// ([`Self::callback_may_open_strict_window`]), raises nothing either, so
    /// the stale value is not pointed out at run time.
    fn callback_runs_untracked_without_window(
        &self,
        primitive: Primitive,
        argument: usize,
    ) -> bool {
        let _ = (primitive, argument);
        false
    }

    /// Whether the callback at `argument` clears tracking but keeps the strict
    /// -read window of the call that runs it (ADR 0443, `runWithOwner`). In a
    /// tracked computation such a call has no window, so the callback's read
    /// is untracked and unwarned.
    fn callback_clears_tracking_in_calling_window(
        &self,
        primitive: Primitive,
        argument: usize,
    ) -> bool {
        let _ = (primitive, argument);
        false
    }

    /// The member a context must be accessed through to act as a provider,
    /// if this dialect has one. Solid 1.x exposes `.Provider`, whose value
    /// getter runs untracked; Solid 2.0 makes the context itself the
    /// provider, so there is no member to name.
    fn context_provider_member(&self) -> Option<&'static str> {
        None
    }

    /// Whether the setter `createOptimistic` returns meets the owned-scope
    /// write guard. The default claims no guard, which only withholds writes.
    fn optimistic_signal_setter_guarded(&self) -> bool {
        false
    }

    /// Whether the pinned compiler lowers an un-namespaced attribute of this
    /// name on an intrinsic element as an event listener. A name it does not
    /// lower is a plain attribute, never a listener. The default claims no
    /// listener, which only withholds handler findings.
    fn compiler_lowers_event_attribute(&self, _name: &str) -> bool {
        false
    }

    /// Whether a statically known string/number in a native `on*` JSX
    /// position is emitted as an attribute instead of installed as a listener.
    /// Solid 1.x's compiler makes that node/value distinction; the shared
    /// handler-value rule must therefore leave those expressions alone rather
    /// than describe them as runtime listeners.
    fn static_event_values_are_attributes(&self) -> bool {
        false
    }

    /// Whether component props are only reactive when a caller passes a
    /// reactive expression — so the engine must prove signal-backing from the
    /// component's call sites instead of assuming every prop is reactive.
    ///
    /// rc.0 ground truth (probed on the published `solid-js@2.0.0-rc.0` dev
    /// entry): `devComponent` wraps the component body in
    /// `untrack(() => Comp(props), '<Name>')`, and the strict-read warning
    /// fires only when a prop *getter* reads reactive state during that
    /// window. A prop every call site passes as a static value compiles to a
    /// plain property and never warns, so reporting its reads would claim a
    /// runtime misbehavior that cannot occur. The 1.x catalog keeps the
    /// upstream over-approximation for eslint-plugin-solid parity, so the
    /// default is `false`.
    fn props_require_caller_proof(&self) -> bool {
        false
    }

    /// Whether the after-await rule also proves member reads (store paths,
    /// component props) after the dominating await, in addition to accessor
    /// calls. The 2.0 rule page claims them; 1.x parity pins the accessor-call
    /// surface upstream's `reactivity` rule counts, so the default is `false`.
    fn reports_member_reads_after_await(&self) -> bool {
        false
    }

    /// Whether this dialect's compiler includes server functions —
    /// the `"use server"` directive, `@solidjs/web/server-functions`, and the
    /// plain-JSON argument transport. Gates the server-function rules
    /// (`valid-use-server`, `server-function-rich-argument`)
    /// so a same-spelled directive in a 1.x project stays out of that
    /// catalog. Solid 1.x has no core server functions, so the default is
    /// `false`.
    fn models_server_functions(&self) -> bool {
        false
    }

    /// The modules this dialect exports `name` from, in `position`.
    ///
    /// The other half of [`Dialect::modules`], and the reason that list is
    /// per-subpath rather than a single package name. `createStore` is the
    /// example: 1.x exports it from `solid-js/store` and importing it from
    /// `solid-js` is an error, while 2.0 folded the store API into core and
    /// the subpath does not exist at all.
    ///
    /// Three properties this has and its predecessor did not, each of which
    /// was a defect:
    ///
    /// - It takes a **name**, not a [`Primitive`]. The vocabulary admits a name
    ///   only when the checker models a reactive obligation for it, so it holds
    ///   40 of 1.x's names and none of the ten under `solid-js/web`. Import
    ///   location is a different question and gets its own index.
    /// - It answers with **every** module, not one. 1.x's `solid-js/web`
    ///   re-exports nine control-flow components, so `Show` resolves from two
    ///   modules and the single-module shape had to pick a wrong one.
    /// - It has **no fallback**. The old implementation answered `solid-js` for
    ///   anything outside a hardcoded arm, so adding `Portal` to the vocabulary
    ///   would have reported correct `solid-js/web` imports as wrong.
    ///
    /// Empty means the dialect does not export the name anywhere, which is a
    /// different answer from "from the package root" and must stay silent.
    fn export_modules(&self, name: &str, position: ExportPosition) -> Vec<&'static str>;

    /// The semantic role of one compiler-resolved exported type.
    ///
    /// Both the alias name and the module must agree with the generated type
    /// export index. A textual type name by itself is never evidence.
    fn type_role(&self, origin_module: &str, name: &str) -> Option<TypeRole> {
        if !self
            .export_modules(name, ExportPosition::Type)
            .contains(&origin_module)
        {
            return None;
        }
        match name {
            "Owner" => Some(TypeRole::Owner),
            "Accessor" | "SourceAccessor" | "Resource" | "InitializedResource" => {
                Some(TypeRole::Accessor)
            }
            "Signal" => Some(TypeRole::Signal),
            "Store" => Some(TypeRole::Store),
            "Setter" => Some(TypeRole::Setter),
            "StoreSetter" => Some(TypeRole::StoreSetter),
            "Component"
            | "ContextProviderComponent"
            | "FlowComponent"
            | "ParentComponent"
            | "VoidComponent" => Some(TypeRole::Component),
            _ => None,
        }
    }

    /// The primitives a namespace import of `module` makes reachable, as in
    /// `import * as solid from "solid-js"` then `solid.createSignal(...)`.
    ///
    /// Enumerated rather than tested, because the caller has to synthesise a
    /// `symbol::name` entry for each one.
    ///
    /// Module-scoped on purpose. 1.x splits its exports across four subpaths
    /// and `createStore` is reachable only through `solid-js/store`; a flat
    /// per-dialect set cannot say that.
    fn namespace_import_primitives(&self, module: &str) -> &'static [&'static str];

    /// Whether a declaration named `name`, resolved to a file inside this
    /// dialect's packages, is one of its primitives.
    ///
    /// This is [`Dialect::primitive`] membership, and deliberately not
    /// [`Dialect::namespace_import_primitives`] membership. The two used to be
    /// the same list, which quietly made the namespace-import set the gate on
    /// *every* declaration site: a name in the vocabulary but absent from that
    /// list resolved nowhere, so adding it to the table alone did nothing at
    /// all. They answer different questions and no longer share an answer.
    fn declares_primitive(&self, name: &str) -> bool {
        self.primitive(name).is_some()
    }

    /// Whether this dialect owns the module a name was imported from.
    fn owns_module(&self, module: &str) -> bool {
        self.modules().contains(&module)
    }

    /// Whether a tag opens the async boundary specifically.
    fn is_async_boundary(&self, tag: &str) -> bool {
        self.boundary_kind(tag) == Some(Boundary::Async)
    }
}
