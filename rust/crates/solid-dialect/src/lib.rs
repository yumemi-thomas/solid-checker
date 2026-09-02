//! The Solid dialect: one interface over the two language versions the checker
//! certifies.
//!
//! Everything version-specific about Solid's *vocabulary* lives here — which
//! names are primitives, which argument of a call is its callback, which JSX
//! tags open a boundary, which primitives may not be created under a leaf
//! owner. The reactive engine asks; it does not know.
//!
//! See ADR 0006, "Reopen the Solid version seam". The decision this crate
//! exists to make possible: one engine, two dialects, rather than one engine
//! per branch.
//!
//! # What does not belong here
//!
//! Syntax. A JSX attribute is a JSX attribute in both dialects, so
//! `solid-ast-facts` takes no dialect. If a rule needs dialect knowledge at the
//! syntax layer, the rule is in the wrong tier.

#![forbid(unsafe_code)]

pub mod exports;
mod solid_1x;
mod solid_2;

pub use exports::Position as ExportPosition;
pub use solid_1x::Solid1x;
pub use solid_2::Solid2;

/// Which Solid language version a project targets.
///
/// Distinct from every wire and schema version in this repository. Type Facts
/// v2, the execution-facts protocol, and `solid-reactivity.json` all carry
/// version numbers of their own that have nothing to do with the Solid
/// version — see ADR 0006, trap 1.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Version {
    /// Solid 1.x — `Suspense`, `createResource`, `createEffect(fn)`.
    V1,
    /// Solid 2.0 — `Loading`, `createTrackedEffect`,
    /// `createEffect(compute, apply)`.
    V2,
}

impl Version {
    /// The adapter for this version.
    #[must_use]
    pub fn dialect(self) -> &'static dyn Dialect {
        match self {
            Self::V1 => &Solid1x,
            Self::V2 => &Solid2,
        }
    }

    /// The dialect a resolved `solid-js` version speaks.
    ///
    /// Takes the major component of a semver string and nothing else, so
    /// `2.0.0-rc.0` is 2.0 and `1.9.14` is 1.x. Prerelease and build metadata
    /// are ignored deliberately: refusing to classify a 2.0 prerelease would
    /// leave real RC projects on the fallback. Range prefixes (`^`, `~`,
    /// `>=`, `<`) are stripped for
    /// the same reason, although the detection path only ever passes exact
    /// installed versions.
    ///
    /// Answers `None` rather than guessing when the string is not a version
    /// or names a major nobody has released. A caller that cannot resolve the
    /// package has to choose its own default; this type will not choose one
    /// for it, because "no solid-js found" and "solid-js 3" deserve different
    /// answers and a default here would erase the difference.
    #[must_use]
    pub fn for_solid_js(version: &str) -> Option<Self> {
        match version
            .trim()
            .trim_start_matches(['^', '~', '=', 'v', ' ', '>', '<'])
            .split(['.', '-', '+'])
            .next()?
            .parse::<u32>()
            .ok()?
        {
            1 => Some(Self::V1),
            2 => Some(Self::V2),
            _ => None,
        }
    }
}

/// A Solid primitive the checker models.
///
/// The union of both dialects. A dialect recognizes a subset: asking
/// [`Dialect::primitive`] for a name the dialect does not export yields
/// `None`, which is how `flush` stays unknown in 1.x and `batch` in 2.0.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum Primitive {
    // Reactive state — both dialects
    CreateSignal,
    CreateMemo,
    CreateStore,
    CreateEffect,
    CreateRenderEffect,
    CreateReaction,
    CreateRoot,
    Untrack,
    OnCleanup,
    MapArray,
    Children,
    /// Creates a context in both dialects. The provider spelling differs:
    /// Solid 1.x exposes `.Provider`, whose value getter runs untracked;
    /// Solid 2.0 makes the context itself the provider.
    CreateContext,

    // 1.x only
    CreateComputed,
    CreateDeferred,
    CreateSelector,
    CreateResource,
    Batch,
    CreateDynamic,
    From,
    On,
    StartTransition,
    UseTransition,
    OnMount,
    OnError,
    CatchError,
    IndexArray,
    MergeProps,
    SplitProps,
    Produce,
    Unwrap,
    CreateMutable,
    ModifyMutable,
    WebMemo,

    // Spelled the same in both dialects: 2.0 kept these 1.x names, and both
    // vocabulary tables map them.
    Hydrate,
    Render,
    GetOwner,
    RunWithOwner,
    Reconcile,

    // 2.0 only
    CreateTrackedEffect,
    CreateProjection,
    CreateOptimistic,
    CreateOptimisticStore,
    CreateOwner,
    Flush,
    OnSettled,
    Action,
    Merge,
    Refresh,
    Affects,
    Dynamic,
    ClientOnly,
    UseHead,
    /// `httpStatus(code, text?)` from `@solidjs/web` — declares the response
    /// status for the calling reactive scope's lifetime during SSR. A
    /// shell-time API: `@solidjs/web@2.0.0-rc.0` `dist/server.js` gates both
    /// the write and the cleanup-time retraction on `!response.committed`,
    /// so a call made after the shell flush is a silent no-op.
    HttpStatus,
    /// `httpHeader(name, value, options?)` from `@solidjs/web`; the same
    /// committed-gate contract as [`Primitive::HttpStatus`].
    HttpHeader,

    // Control flow — component tags
    For,
    Show,
    Switch,
    Match,
    Index,
    Repeat,
    Suspense,
    SuspenseList,
    ErrorBoundary,
    Loading,
    // Solid 2.0 additions extracted from solid-js@2.0.0-rc.0 and its
    // bundled @solidjs/signals runtime (ADR 0006's rule: the package, not the
    // docs). The engine modelled none of these before.
    /// Also a 1.x core export by the same name; only the 2.0 table maps it
    /// today, and the 1.x dialect leaves it unmodelled (inert, since the
    /// engine keys nothing on it besides `CreateContext`).
    UseContext,
    CreateErrorBoundary,
    CreateLoadingBoundary,
    Latest,
    IsPending,
    Resolve,
    Omit,
    Deep,
    Snapshot,
    /// `<Errored>`, 2.0's error-boundary component. Distinct from
    /// [`Primitive::ErrorBoundary`], which is 1.x's spelling of the same role;
    /// [`Dialect::boundary_kind`] is where the two meet.
    Errored,
    /// `lazy(() => import("./Comp"))` — both dialects' vocabulary, with the
    /// same shape and different timing. In 2.0 the loader is called in place
    /// inside the wrapper component, so it inherits that owner; in 1.x the
    /// loader is stored on the returned component and invoked only when that
    /// component first renders (or its `preload` method is called). In both,
    /// the result is awaited and memoised, so nothing in the loader
    /// subscribes.
    Lazy,
    /// `createRevealOrder(fn, options?)`. Creates an owner and runs `fn` under
    /// it, coordinating the reveal timing of sibling loading boundaries.
    CreateRevealOrder,
    /// `repeat(count, mapFn)`, the function. Distinct from
    /// [`Primitive::Repeat`], which is the `<Repeat>` component — 2.0 exports
    /// both, and a primitive maps to exactly one name per dialect.
    RepeatMap,
}

/// Owner-requirement category carried by an exact primitive call. Kept in the
/// dialect vocabulary so proof adapters never reconstruct Solid behavior from
/// a function name in shared infrastructure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerRequirementRole {
    Effect,
    Cleanup,
    SettledCleanup,
}

/// Returns an owner-requirement role only when every dialect that recognizes
/// the exact export name assigns the same role. An unrecognized or disputed
/// name stays open rather than inheriting behavior from another dialect.
#[must_use]
pub fn unambiguous_owner_requirement_role(name: &str) -> Option<OwnerRequirementRole> {
    let mut roles = [Version::V1, Version::V2]
        .into_iter()
        .filter_map(|version| {
            let dialect = version.dialect();
            let primitive = dialect.primitive(name)?;
            (dialect.name_of(primitive) == Some(name)).then_some(primitive)
        })
        .filter_map(|primitive| match primitive {
            Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateTrackedEffect => Some(OwnerRequirementRole::Effect),
            Primitive::OnCleanup => Some(OwnerRequirementRole::Cleanup),
            Primitive::OnSettled => Some(OwnerRequirementRole::SettledCleanup),
            _ => None,
        });
    let first = roles.next()?;
    roles.all(|role| role == first).then_some(first)
}

/// Returns true only when every dialect that canonically exports `name`
/// identifies `argument` as a callback for this exact call shape.
#[must_use]
pub fn unambiguous_callback_argument(name: &str, argument: usize, argument_count: usize) -> bool {
    let answers = [Version::V1, Version::V2]
        .into_iter()
        .filter_map(|version| {
            let dialect = version.dialect();
            let primitive = dialect.primitive(name)?;
            (dialect.name_of(primitive) == Some(name))
                .then(|| dialect.callback_execution_at(primitive, argument, argument_count))
        })
        .collect::<Vec<_>>();
    let Some(Some(first)) = answers.first().copied() else {
        return false;
    };
    answers.into_iter().all(|answer| answer == Some(first))
}

/// Returns true only when the exact public type export is function-shaped in
/// every dialect that recognizes it from this module.
#[must_use]
pub fn unambiguous_callable_type(origin_module: &str, name: &str) -> bool {
    let roles = [Version::V1, Version::V2]
        .into_iter()
        .filter_map(|version| version.dialect().type_role(origin_module, name))
        .collect::<Vec<_>>();
    !roles.is_empty()
        && roles
            .into_iter()
            .all(|role| matches!(role, TypeRole::Accessor | TypeRole::Setter))
}

/// Returns true only for a tuple item whose callability is fixed by every
/// dialect that canonically exports the exact primitive name.
#[must_use]
pub fn unambiguous_callable_result_tuple_item(name: &str, index: usize) -> bool {
    let answers = [Version::V1, Version::V2]
        .into_iter()
        .filter_map(|version| {
            let dialect = version.dialect();
            let primitive = dialect.primitive(name)?;
            (dialect.name_of(primitive) == Some(name)).then_some(match primitive {
                Primitive::CreateSignal => matches!(index, 0 | 1),
                _ => false,
            })
        })
        .collect::<Vec<_>>();
    !answers.is_empty() && answers.into_iter().all(|answer| answer)
}

/// The role a JSX tag plays as a boundary.
///
/// Callers ask for the role, never the name: 1.x spells the async boundary
/// `Suspense` and 2.0 spells it `Loading`, and no rule should have to know
/// which.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Boundary {
    /// Bounds pending async reads.
    Async,
    /// Catches thrown errors.
    Error,
}

/// Semantic identities of public Solid types used by shared analysis.
///
/// The engine asks for a role only after Type Facts has proved both the alias
/// declaration and its origin module. This keeps exported spellings in the
/// dialect vocabulary and prevents a same-named user alias from becoming a
/// reactive source or component.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TypeRole {
    Component,
    Owner,
    Accessor,
    Signal,
    Store,
    Setter,
    StoreSetter,
}

/// Whether creating a primitive inside a leaf owner is forbidden.
///
/// The conditional case is real and load-bearing in Solid 2.0:
/// `createSignal(fn)` registers a derived computation while `createSignal(0)`
/// does not. Solid 1.x stores the function as data and therefore answers
/// [`CleanupRule::Never`] for the same call shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanupRule {
    /// Never allowed in a leaf-owner or cleanup scope.
    Always,
    /// Forbidden only when the first argument is a function.
    WhenFirstArgumentIsFunction,
    /// Not restricted.
    Never,
}

/// Which owner a primitive's callback argument runs under.
///
/// Distinct from [`Dialect::callback_positions`], which answers *where* a
/// callback sits so its reads can be classified. This answers who disposes
/// what the callback creates, and the two disagree often enough that
/// conflating them is a bug: `untrack(fn)` and `createRoot(fn)` both take a
/// callback at index 0, and an effect created inside the first is an orphan
/// while one created inside the second is not.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallbackOwner {
    /// The callback runs under a definite owner this primitive establishes.
    ///
    /// Usually one it creates and later disposes — `createRoot`. At a concrete
    /// `runWithOwner(owner, fn)` call this also covers a supplied owner proven
    /// non-null: the distinction between making and supplying it matters to
    /// the runtime, while callers here ask whether the callback has one.
    Creates,
    /// The callback may run with or without an owner depending on a runtime
    /// value. `runWithOwner(owner, fn)` has this shape whenever `owner` is
    /// nullable and the call site cannot prove which branch it holds.
    Conditional,
    /// Whatever owner the *call* runs under. The callback adds no scope of
    /// its own, so an effect inside it is exactly as owned as the call site.
    Inherits,
    /// No owner at all, whatever the call site.
    None,
    /// An owner that cannot hold cleanup — a leaf.
    Leaf,
}

/// How a control-flow component's `keyed` prop was written.
///
/// It decides which of the children callback's parameters are accessors, which
/// is why it is a shape and not a `bool`: 2.0's `<For keyed={item => item.id}>`
/// makes *both* parameters accessors, and neither `true` nor `false` describes
/// it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyForm {
    /// No `keyed` prop at all.
    Absent,
    /// `keyed` or `keyed={true}`.
    Keyed,
    /// `keyed={false}`.
    Unkeyed,
    /// `keyed={expression}` where the expression is *proven* a function — an
    /// inline function literal, or a value whose type facts say callable.
    CustomKey,
    /// `keyed={expression}` where the expression is a boolean (or cannot be
    /// resolved): the flag's runtime truthiness picks the keyed or unkeyed
    /// overload, so the children callback's shape is ambiguous. RFC 03 warns
    /// against exactly this ("Avoid dynamic boolean `keyed` values with
    /// function children ... prefer a literal `true`, literal `false`, or a
    /// custom key function"). A static table cannot prove which overload
    /// runs, and claiming an accessor for what may be a raw value would
    /// fabricate a source — so this form claims nothing.
    DynamicFlag,
}

/// When a primitive's callback argument runs, and whether it re-runs.
///
/// The third question about a callback argument, orthogonal to the other two:
/// [`Dialect::callback_positions`] says *where* it sits, [`CallbackOwner`] says
/// who disposes what it creates, and this says *when it runs*. `untrack(fn)`
/// and `createRoot(fn)` share a position and differ in owner; `untrack(fn)` and
/// `createMemo(fn)` share a position and differ here.
///
/// These three analyzer roles are projections of the normalized contract's
/// independent tracking, event, and schedule axes. The checked bundle
/// authorities hold the tables below to the exact published package behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Execution {
    /// The callback creates its own observer: reads inside it subscribe *it*,
    /// and it re-runs when one of them changes.
    Tracked,
    /// The callback runs outside the caller's tracking pass: reads inside it
    /// subscribe nothing the caller owns. Usually it also runs later than the
    /// call — on the next tick, on cleanup, when a resource settles — but the
    /// attribution is the claim.
    Deferred,
    /// The callback runs inside the caller's tracking pass: reads inside it
    /// subscribe whatever was tracking at the call site, and it does not
    /// re-run on its own.
    ///
    /// A primitive that clears the listener while staying inline — `untrack`,
    /// `createRoot`, `runWithOwner` — says so through
    /// [`Dialect::runs_callback_deferred`] instead of through a different
    /// execution, because the two facts are independent and consumers ask
    /// about them separately.
    Inline,
}

// These three classify **attribution**, and the distinction from timing is
// load-bearing rather than pedantic. There are two consumers, they ask
// different questions, and only one of them is answered by this word alone.
//
// `callback_runs_outside_tracking` in solid-reactive-ir is the attribution
// consumer: Deferred is "outside the current tracking pass" unconditionally,
// Inline "inherits the caller's Listener" unless the primitive is separately
// marked as listener-clearing, and Tracked creates its own observer unless
// `tracks_reads` overrides.
//
// 1.x `startTransition` is the case that proves attribution is the right axis
// *for that consumer*. Its callback runs in a `Promise.resolve().then()`
// microtask, so by timing it is plainly not immediate — and it is Inline,
// correctly, because the runtime restores the captured Listener around it and a
// read inside subscribes exactly as at the call site. Probed: `batch`,
// `catchError`'s first argument and `startTransition` all subscribe an
// enclosing memo; `untrack` and `createRoot`, both listener-clearing, do not;
// `createResource`'s fetcher does not, which is why it is Deferred even though
// the sourced overload runs it during the call. Classifying *that* consumer by
// timing would move `startTransition` to Deferred and tell the engine that
// reads inside it escape the caller's scope, which the runtime contradicts.
//
// **Package-contract emission is the second consumer, and it does ask when the
// callback ran.** `callback_wrapper_at` (solid-reactive-ir/src/interproc.rs)
// reads these same rows to compose an `execution` row for an export, and a
// contract row is a promise a probe measures against the clock: `inline`
// promises the export invoked the callback before returning, `deferred`
// promises it did not. So emission never reads the schedule *out of* this word.
// It reads the word for attribution and takes the schedule from separate
// dialect facts — [`Dialect::runs_callback_synchronously`] for the
// listener-clearing primitives that nonetheless run during the call, and
// [`Dialect::tracked_callback_timing`] for when a tracked computation runs
// relative to the call that creates it. Where a dialect states no schedule
// fact, emission publishes no row rather than reading one off the word.
//
// The two divergences above are exactly where the readings differ, and both are
// closed by that split rather than papered over: `startTransition` and
// `createResource` are absent from `primitive_callback_execution`'s schedule
// table, so contract emission refuses them instead of restating their
// attribution as a schedule.

/// When a primitive's [`Execution::Tracked`] callback runs, relative to the
/// primitive's own call returning.
///
/// Orthogonal to [`Execution`], which says who owns the reads. A tracked
/// computation is tracked either way; this says whether the export that created
/// it has already run it by the time it returns, which is the only thing a
/// package-contract `execution` row can promise about a callback the package
/// detached from tracking. There is deliberately no third member for "never
/// runs": that is the absence of an answer, spelled `None` by
/// [`Dialect::tracked_callback_timing`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackedCallbackTiming {
    /// The computation runs to completion before the creating call returns —
    /// 1.x `createMemo`/`createRenderEffect`, 2.0's `effect()` compute.
    DuringCall,
    /// The creating call only queues the computation, so it has not run when
    /// that call returns — 1.x `createEffect` under any owner, 2.0
    /// `createTrackedEffect`.
    AfterCall,
}

/// The complete callback contract for one argument of one concrete primitive
/// call. Consumers ask one question and receive the execution, ownership,
/// reachability, dormancy, tracking, and callback-parameter source semantics
/// as one coherent answer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallbackSemantics {
    pub execution: Option<Execution>,
    pub owner: Option<CallbackOwner>,
    pub tracks_reads: bool,
    pub requires_return_invocation: bool,
    pub stores_as_value: bool,
    pub accessor_parameters: &'static [usize],
}

/// The callback contract of a function returned by a primitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReturnedCallbackSemantics {
    pub execution: Option<Execution>,
    pub owner: Option<CallbackOwner>,
}

/// One Solid language version's vocabulary.
///
/// Implementors are stateless; a dialect is a set of tables, and every method
/// is a lookup.
pub trait Dialect: Sync {
    /// Which version this adapter speaks.
    fn version(&self) -> Version;

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

    /// The basename diagnostics cite when a fact came from this dialect's
    /// bundled `solid-js` contract (`bundled://<basename>#<primitive>`).
    ///
    /// The label is the checked-in artifact the fact was actually read from,
    /// so a 1.x diagnostic must not cite the 2.0 file.
    fn bundled_contract_label(&self) -> &'static str;

    /// Resolves an exported name to a primitive, or `None` when this dialect
    /// does not export it.
    fn primitive(&self, name: &str) -> Option<Primitive>;

    /// The exported name for a primitive in this dialect.
    fn name_of(&self, primitive: Primitive) -> Option<&'static str>;

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

    /// Whether this primitive clears tracking around a callback it nonetheless
    /// runs **before its own call returns**.
    ///
    /// [`Dialect::runs_callback_deferred`] answers one boolean for two
    /// independent questions — "the listener is cleared" and "it runs later" —
    /// because its attribution consumer, `callback_runs_outside_tracking`, asks
    /// only the first. Package contracts ask the second: an `execution` row is
    /// a promise about *when* the
    /// export invokes a caller-supplied callback, and the contract vocabulary
    /// keeps `untrack`, `createRoot` and `runWithOwner` at `inline` while the
    /// listener-clearing fact travels separately
    /// (docs/package-contracts.md, "callback execution").
    ///
    /// This is the "detached, not later" half, and it is **derived rather than
    /// enumerated** so the two answers cannot drift: exactly the members of
    /// [`Dialect::runs_callback_deferred`] whose own
    /// [`Dialect::callback_executions`] rows are all [`Execution::Inline`]. A
    /// primitive the dialect models no callback for answers `false` — absence
    /// of a row is not evidence of synchrony. `the_synchronous_clearing_set_*`
    /// pins the resulting set per dialect.
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
    /// composes to for package-contract emission. Reading `Tracked` as "runs
    /// later" is false for most of 1.x's own tracked primitives: `createMemo`
    /// and `createRenderEffect` run the computation during the call, and only
    /// `createEffect` queues it.
    ///
    /// **`None` is a refusal, not a default.** It means this dialect has
    /// established no schedule for that callback — because the audited runtime
    /// was not read for it, because the primitive never invokes the argument at
    /// all (1.x `createSignal(fn)` stores it), or because the shape resisted
    /// measurement. Contract emission leaves that exact callback leaf open rather
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

    /// Whether calling this primitive produces a reactive source — an
    /// accessor, a store, or a tuple containing one.
    ///
    /// Source *discovery* is where a rule's chain starts: a read the engine
    /// cannot trace to a source is not a read it can report on. The bundled
    /// contract answers this for single-value returns through its `returns`
    /// column, and that path works; what it cannot express is a tuple, so
    /// `createSignal` and friends have always been answered here instead.
    ///
    /// 1.x has five the contract's column cannot reach — `createResource`
    /// returns `[accessor, { mutate, refetch }]` — and until this was a
    /// dialect question the engine used 2.0's list for both, so a read through
    /// a 1.x resource was traced to nothing and reported nowhere.
    fn creates_reactive_source(&self, primitive: Primitive) -> bool;

    /// Whether creating this primitive registers a directive-applied owner.
    fn creates_directive_owner(&self, primitive: Primitive) -> bool;

    /// Whether the runtime serializes a literal `false` JSX attribute value
    /// by *removing* the attribute on intrinsic elements.
    ///
    /// RFC 07 unified boolean handling — "Boolean literals add/remove the
    /// attribute (no `="true"` string)" — and this is the half of that
    /// sentence the checker still owns. The `true` half is real too (probed
    /// on `@solidjs/web@2.0.0-rc.0`, 2026-08-15: `ssrAttribute("draggable",
    /// true)` → ` draggable`, `setAttribute(el, "draggable", true)` →
    /// `el.setAttribute("draggable", "")`, both selecting `auto`), but 2.0's
    /// published `EnumeratedPseudoBoolean` type rejects `draggable={true}`
    /// and the shorthand outright, so that spelling is TypeScript's to
    /// report and needs no dialect question. From the same probe: the client
    /// `setAttribute`/`assign` paths remove the attribute for `false` and
    /// SSR omits it. For an *enumerated* attribute such as `draggable`,
    /// removal selects the `auto` default rather than the `"false"` state —
    /// on draggable-by-default elements (`img`, `a[href]`) that silently
    /// re-enables dragging.
    ///
    /// 1.x's dom-expressions stringifies instead: `draggable={false}`
    /// renders `draggable="false"` and behaves, so the default answer is
    /// `false` and only the 2.0 dialect opts in.
    fn false_attribute_value_removes_attribute(&self) -> bool {
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
    /// contracts, not TypeScript return types, so source discovery must ask
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
    /// Overlaps the bundled contract's `returns.kind` on purpose, and the
    /// engine consults both. The contract can describe a single returned
    /// store — 1.x's `createMutable`, 2.0's `createProjection` — and cannot
    /// describe a tuple, which is what `createStore` returns in both versions.
    fn returns_store(&self, primitive: Primitive) -> bool;

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

    /// Whether this primitive's options contract includes `sync`.
    ///
    /// An options slot alone is not evidence for a particular option key:
    /// Solid 1.x has options objects for memos, signals, and stores but no
    /// synchronous-node contract. Keeping the key in the dialect prevents a
    /// 2.0-only diagnostic identity from reaching the 1.x rule catalog.
    fn supports_sync_option(&self, primitive: Primitive) -> bool {
        let _ = primitive;
        false
    }

    /// When each callback argument of `primitive` runs.
    ///
    /// Empty means the dialect models no callback for it — the same answer the
    /// bundled contract gives by omitting a `callbacks` column, and not a
    /// claim that no function can be passed.
    ///
    /// The 1.x/2.0 split this exists for: `createEffect` has one tracked
    /// callback in 1.x and a tracked compute plus a deferred apply in 2.0.
    /// Hardcoding the 2.0 pair described a read in 1.x's *seed value* as being
    /// in an "apply callback" that version does not have.
    fn callback_executions(&self, primitive: Primitive) -> &'static [(usize, Execution)];

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
    /// boundary that neither TypeScript overloads nor the package contract's
    /// first-order callback list can express.
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
    /// table is the default and remains checkable against the bundled package
    /// contract; dialects override this method only for overloads whose roles
    /// depend on call shape and therefore cannot be represented by the
    /// contract schema's flat parameter list. Solid 1.x `createResource` is
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
                && !self.runs_callback_deferred(primitive),
            requires_return_invocation: self
                .callback_requires_return_invocation(primitive, argument),
            stores_as_value: self.stores_function_argument_as_value(primitive, argument),
            accessor_parameters: self.callback_accessor_parameters(primitive, argument),
        }
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

    /// The member a context must be accessed through to act as a provider,
    /// if this dialect has one. Solid 1.x exposes `.Provider`, whose value
    /// getter runs untracked; Solid 2.0 makes the context itself the
    /// provider, so there is no member to name.
    fn context_provider_member(&self) -> Option<&'static str> {
        None
    }

    /// Whether this dialect's catalog carries the file-local ESLint-era rule
    /// surface (the `SC8xxx` identities ported from eslint-plugin-solid).
    /// Only the 1.x catalog does; the engine gates those passes on this
    /// answer instead of naming a version.
    fn carries_eslint_era_rules(&self) -> bool {
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

    /// Whether this dialect's compiler contract includes server functions —
    /// the `"use server"` directive, `@solidjs/web/server-functions`, and the
    /// plain-JSON argument transport. Gates the server-function rules
    /// (`server-function-module-directive`, `server-function-rich-argument`)
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

/// A lazily built name → primitive index. One static per dialect.
pub(crate) type NameIndex = std::sync::OnceLock<std::collections::HashMap<&'static str, Primitive>>;

/// Looks a name up across `(name, primitive)` tables through a hash index
/// built once on first use.
///
/// [`Dialect::primitive`] sits on hot engine paths — every call expression
/// and import the engine classifies asks it — and a linear scan of a
/// ~50-entry table there is pure overhead. The tables stay the source of
/// truth (the tests iterate and cross-check them); the map is only the
/// access path, so the semantics are exactly the scan's. Table order cannot
/// matter: the sortedness tests hold each table free of duplicate names, so
/// no entry can shadow another.
pub(crate) fn lookup(
    index: &'static NameIndex,
    tables: &[&'static [(&'static str, Primitive)]],
    name: &str,
) -> Option<Primitive> {
    index
        .get_or_init(|| {
            tables
                .iter()
                .flat_map(|table| table.iter().copied())
                .collect()
        })
        .get(name)
        .copied()
}

/// Reverse of [`lookup`].
pub(crate) fn reverse(
    table: &[(&'static str, Primitive)],
    primitive: Primitive,
) -> Option<&'static str> {
    table
        .iter()
        .find(|(_, candidate)| *candidate == primitive)
        .map(|(name, _)| *name)
}

#[cfg(test)]
fn callback_exports_from_bundles(
    dialect: &str,
    packages: &[&str],
) -> std::collections::BTreeMap<String, Vec<(usize, Execution)>> {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("contracts")
        .join(dialect);
    let mut exports = std::collections::BTreeMap::<String, Vec<(usize, Execution)>>::new();
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json")
            || path.file_name().is_some_and(|name| {
                name.to_string_lossy().contains("receipt") || name == "bundle-index.json"
            })
        {
            continue;
        }
        let contract: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        if !packages.contains(&contract["package"]["name"].as_str().unwrap_or_default()) {
            continue;
        }
        let summaries = contract["summaries"].as_object().unwrap();
        for entrypoint in contract["entrypoints"].as_object().unwrap().values() {
            let cases = entrypoint["cases"]
                .as_array()
                .map_or_else(|| vec![entrypoint], |cases| cases.iter().collect());
            for artifact_case in cases {
                for (name, reference) in artifact_case["exports"].as_object().unwrap() {
                    let summary_id = reference
                        .as_str()
                        .or_else(|| reference["summary"].as_str())
                        .unwrap();
                    let call = &summaries[summary_id]["call"];
                    let rows = exports.entry(name.clone()).or_default();
                    for callback in call["callbacks"].as_array().into_iter().flatten() {
                        let Some(index) = callback["from"]["arg"]
                            .as_u64()
                            .and_then(|index| usize::try_from(index).ok())
                        else {
                            continue;
                        };
                        let operation_id = callback["operation"].as_str().unwrap();
                        let operation = call["operations"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .find(|operation| operation["id"] == operation_id)
                            .unwrap();
                        let execution = if operation["tracking"] == "tracked" {
                            Execution::Tracked
                        } else if operation["at"]["schedule"] == "same-stack" {
                            Execution::Inline
                        } else {
                            Execution::Deferred
                        };
                        if !rows.contains(&(index, execution)) {
                            rows.push((index, execution));
                        }
                    }
                }
            }
        }
    }
    exports
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialects() -> [&'static dyn Dialect; 2] {
        [Version::V1.dialect(), Version::V2.dialect()]
    }

    #[test]
    fn semantic_type_roles_require_an_exact_export_and_module() {
        for dialect in dialects() {
            assert_eq!(
                dialect.type_role("solid-js", "Accessor"),
                Some(TypeRole::Accessor)
            );
            assert_eq!(
                dialect.type_role("solid-js", "Component"),
                Some(TypeRole::Component)
            );
            assert_eq!(dialect.type_role("user-module", "Accessor"), None);
            assert_eq!(dialect.type_role("solid-js", "ComponentProps"), None);
        }
        assert_eq!(
            Version::V1.dialect().type_role("solid-js", "Resource"),
            Some(TypeRole::Accessor)
        );
        assert_eq!(
            Version::V1.dialect().type_role("solid-js/store", "Store"),
            Some(TypeRole::Store)
        );
        assert_eq!(
            Version::V2.dialect().type_role("solid-js", "Signal"),
            Some(TypeRole::Signal)
        );
    }

    /// [`Dialect::callback_executions`] is a projection of receipt-issued
    /// normalized first-party semantics. Both describe the same package; a
    /// name they disagree about means one of them was edited alone.
    ///
    /// Read from the generated contracts rather than the generator's source,
    /// because the contract is what the checker actually loads — and because a
    /// crate below `solid-facts-backend` cannot depend on it. The files are
    /// parsed with `serde_json`, a dev-dependency that exists for this.
    ///
    /// Names whose dialect-modelled overload behavior is more call-site
    /// specific than the selected first-party artifact case, exempted from the
    /// reverse direction below. Every entry needs a reason:
    ///
    /// - 2.0 `createSignal`/`createStore`/`createOptimistic`/
    ///   `createOptimisticStore`: the derived `createX(fn, …)` forms branch on
    ///   `typeof first === "function"` at runtime; the contract describes the
    ///   value form, which takes no callback.
    /// - 2.0 `dynamic`: the browser implementation owns a tracked memo. The
    ///   root server helper is eager, while the JSX runtime's lazy memo defers
    ///   the same source. Exact behavior remains local to the artifact case.
    /// - 2.0 `clientOnly`: browser and server artifact cases intentionally
    ///   disagree about whether the loader is invoked on the same stack.
    /// - 2.0 `latest`/`isPending`: the analyzer's callback position denotes a
    ///   reactive accessor input; the normalized contract models it as a read
    ///   operation rather than invocation of a caller-supplied callback.
    /// - 1.x `on`: the dialect's `(0, Inline)` row is an engine keying — the
    ///   returned adapter's invocation site decides the role (see
    ///   `callback_executions`' comment) — not a package fact the review
    ///   contract should record as `inline`.
    /// - 1.x `createResource`: parameter 0 is a tracked source in the sourced
    ///   overload and the deferred fetcher in the unsourced overload. The
    ///   dialect resolves that overload at the call site.
    fn contract_schema_exemptions(version: Version, name: &str) -> bool {
        match version {
            Version::V1 => matches!(name, "createResource" | "on"),
            Version::V2 => matches!(
                name,
                "createSignal"
                    | "createStore"
                    | "createOptimistic"
                    | "createOptimisticStore"
                    | "clientOnly"
                    | "dynamic"
                    | "isPending"
                    | "latest"
            ),
        }
    }

    /// Two-directional since the contracts gained their missing rows. The
    /// forward direction is the old one: a contract row the dialect
    /// contradicts means one of them was edited alone. The reverse direction
    /// closes the gap that let `effect`/`memo` sit in a contract with no
    /// `callbacks` column while the dialect modelled both: a name the dialect
    /// models callbacks for must carry those rows in some contract file of
    /// its version, unless [`contract_schema_exemptions`] records why the
    /// flat schema cannot express the fact.
    #[test]
    fn the_callback_executions_agree_with_the_bundled_contract() {
        let mut checked = 0;
        for (version, bundle, packages) in [
            (Version::V1, "solid-v1", &["solid-js"][..]),
            (Version::V2, "solid-v2", &["solid-js", "@solidjs/web"][..]),
        ] {
            let dialect = version.dialect();
            let contract_rows = callback_exports_from_bundles(bundle, packages);
            for (name, rows) in &contract_rows {
                let Some(primitive) = dialect.primitive(name) else {
                    continue;
                };
                if contract_schema_exemptions(version, name) {
                    continue;
                }
                let modelled = dialect.callback_executions(primitive);
                if modelled.is_empty() {
                    continue;
                }
                for (index, expected) in rows {
                    assert!(
                        modelled.contains(&(*index, *expected)),
                        "{bundle}: {name} argument {index} is {expected:?} in the contract, and the {version:?} dialect says {modelled:?}"
                    );
                    checked += 1;
                }
            }
            let mut missing = Vec::new();
            for (name, rows) in &contract_rows {
                if contract_schema_exemptions(version, name) {
                    continue;
                }
                let Some(primitive) = dialect.primitive(name) else {
                    continue;
                };
                for entry in dialect.callback_executions(primitive) {
                    if !rows.contains(entry) {
                        missing.push(format!("{name} {entry:?}"));
                    }
                }
            }
            assert!(
                missing.is_empty(),
                "{version:?} dialect models callbacks its contract files do not carry \
                 (add the rows, or record a schema exemption with a reason): {missing:?}"
            );
        }
        // A silent zero here would make the assertion above unreachable and
        // this test a no-op, which is how the sidecar protocol check rotted.
        assert!(checked > 20, "only {checked} callbacks cross-checked");
    }

    #[test]
    fn every_recognized_name_round_trips() {
        for dialect in dialects() {
            for name in dialect_names(dialect).iter().copied() {
                let primitive = dialect
                    .primitive(name)
                    .unwrap_or_else(|| panic!("{name} resolves in {:?}", dialect.version()));
                let canonical = dialect.name_of(primitive).unwrap_or_else(|| {
                    panic!(
                        "{name} has no canonical spelling in {:?}",
                        dialect.version()
                    )
                });
                assert_eq!(
                    dialect.primitive(canonical),
                    Some(primitive),
                    "{name} canonicalizes to {canonical}, which does not resolve back in {:?}",
                    dialect.version()
                );
            }
        }
    }

    #[test]
    fn callback_positions_are_the_dialects_headline_difference() {
        let one = Version::V1.dialect();
        let two = Version::V2.dialect();

        // 1.x: createEffect(fn, value?) — the callback is first, and index 1 is
        // a seed VALUE. 2.0: createEffect(compute, apply) — index 1 is the
        // apply callback. Reading 1.x's seed as a callback is the single
        // highest-yield way to get this wrong.
        assert_eq!(one.callback_positions(Primitive::CreateEffect), &[0]);
        assert_eq!(two.callback_positions(Primitive::CreateEffect), &[1]);
        assert_eq!(one.callback_positions(Primitive::CreateRenderEffect), &[0]);
        assert_eq!(two.callback_positions(Primitive::CreateRenderEffect), &[1]);

        // Unchanged across the split.
        assert_eq!(one.callback_positions(Primitive::CreateMemo), &[0]);
        assert_eq!(two.callback_positions(Primitive::CreateMemo), &[0]);
        assert_eq!(one.callback_positions(Primitive::Untrack), &[0]);
        assert_eq!(two.callback_positions(Primitive::Untrack), &[0]);
    }

    #[test]
    fn deferred_execution_is_independent_of_callback_position() {
        let two = Version::V2.dialect();

        // The pair that makes this a separate question. Both put a callback at
        // index 0; one defers, one tracks. A rule that inferred "deferred"
        // from position would stop reporting reads inside every memo compute.
        assert_eq!(two.callback_positions(Primitive::Untrack), &[0]);
        assert_eq!(two.callback_positions(Primitive::CreateMemo), &[0]);
        assert!(two.runs_callback_deferred(Primitive::Untrack));
        assert!(!two.runs_callback_deferred(Primitive::CreateMemo));

        // 2.0 defers these imperative or loader callbacks even though each is
        // reachable from the call.
        let deferred = [
            Primitive::Flush,
            Primitive::Untrack,
            Primitive::OnSettled,
            Primitive::CreateReaction,
            Primitive::Action,
            Primitive::ClientOnly,
        ];
        for primitive in deferred {
            assert!(two.runs_callback_deferred(primitive), "{primitive:?}");
        }
        for primitive in [
            Primitive::CreateTrackedEffect,
            Primitive::CreateSignal,
            Primitive::CreateStore,
            Primitive::CreateProjection,
            Primitive::CreateOptimistic,
            Primitive::CreateOptimisticStore,
            Primitive::Dynamic,
            // latest(fn) and isPending(fn) catch NotReadyError but do NOT
            // clear tracking: reads inside them subscribe in the caller's
            // scope, so classifying them as deferred would erase those read
            // obligations.
            Primitive::Latest,
            Primitive::IsPending,
        ] {
            assert!(!two.runs_callback_deferred(primitive), "{primitive:?}");
        }

        // Solid 1.x batch is synchronous and startTransition restores the
        // captured Listener before invoking its callback. Neither explicitly
        // clears tracking; createRoot does. (Timing caveat: the 1.9 runtime
        // invokes the transition callback in a Promise.resolve().then()
        // microtask, but tracking-wise it behaves like the call site, which
        // is what this question asks.)
        let one = Version::V1.dialect();
        assert!(!one.runs_callback_deferred(Primitive::Batch));
        assert!(!one.runs_callback_deferred(Primitive::StartTransition));
        assert!(one.runs_callback_deferred(Primitive::CreateRoot));
        assert!(!one.runs_callback_deferred(Primitive::CreateMemo));
    }

    /// The set every synchronous-clearing name resolves to, per dialect.
    fn synchronous_clearing_names(dialect: &'static dyn Dialect) -> Vec<&'static str> {
        let mut names = dialect_names(dialect)
            .into_iter()
            .filter(|name| {
                dialect
                    .primitive(name)
                    .is_some_and(|primitive| dialect.runs_callback_synchronously(primitive))
            })
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// `runs_callback_synchronously` is derived, so this test is not checking
    /// an enumeration against itself: it pins the *concrete* set the derivation
    /// produces, which is what package contracts publish as `inline`. A row
    /// moving into or out of `callback_executions`, or a primitive joining
    /// `runs_callback_deferred`, changes contract bytes for every package that
    /// forwards a callback through it, and has to be a deliberate edit here.
    #[test]
    fn the_synchronous_clearing_set_is_the_inline_half_of_the_deferred_set() {
        let one = Version::V1.dialect();
        let two = Version::V2.dialect();

        assert_eq!(
            synchronous_clearing_names(one),
            vec!["createRoot", "runWithOwner", "untrack"]
        );
        // `flush` earns its place on the rc runtime's own bytes, not on its
        // name: `@solidjs/signals` `flush(fn)` runs `fn()` inside a
        // `try { return fn() } finally { flush(); syncDepth-- }`, so the
        // callback is invoked and returned from *during* the call
        // (2.0.0-rc dev bundle, `flush`). `createRevealOrder` is here for the
        // same reason `createRoot` is — it clears tracking while establishing
        // an owner and runs its callback immediately.
        assert_eq!(
            synchronous_clearing_names(two),
            vec![
                "createRevealOrder",
                "createRoot",
                "flush",
                "runWithOwner",
                "untrack"
            ]
        );

        // The two halves of `runs_callback_deferred` stay separable: a
        // genuinely later callback is never synchronous, and a primitive the
        // dialect models no callback for is never either.
        for primitive in [Primitive::OnCleanup, Primitive::CreateReaction] {
            assert!(!one.runs_callback_synchronously(primitive), "{primitive:?}");
        }
        for primitive in [Primitive::OnSettled, Primitive::Action, Primitive::Lazy] {
            assert!(!two.runs_callback_synchronously(primitive), "{primitive:?}");
        }
        // Inline but not listener-clearing: `batch` is transparent to its call
        // site, so it is not in this set either.
        assert!(!one.runs_callback_synchronously(Primitive::Batch));
        assert!(!two.runs_callback_synchronously(Primitive::Latest));
    }

    /// Every name whose tracked callback this dialect gives `timing` for, at
    /// any argument index a two-argument call could carry.
    fn tracked_schedule_names(
        dialect: &'static dyn Dialect,
        timing: TrackedCallbackTiming,
    ) -> Vec<&'static str> {
        let mut names = dialect_names(dialect)
            .into_iter()
            .filter(|name| {
                dialect.primitive(name).is_some_and(|primitive| {
                    (0..2).any(|argument| {
                        dialect.tracked_callback_timing(primitive, argument, 2) == Some(timing)
                    })
                })
            })
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// The eager/deferring/unestablished partition, pinned per dialect.
    ///
    /// These sets decide what a clearing wrapper *inside* a tracked one
    /// publishes, so a name entering or leaving one changes contract bytes for
    /// every package with that shape. The unestablished side is pinned too: it
    /// is the fail-closed arm, and silently promoting a member out of it is how
    /// a guessed schedule would ship.
    #[test]
    fn the_tracked_callback_schedule_partitions_each_dialect() {
        let one = Version::V1.dialect();
        let two = Version::V2.dialect();

        // 1.x: everything that reaches `updateComputation` on the creating
        // call, and `createEffect`, which pushes onto `Effects` instead.
        // Source lines in `Solid1x::tracked_callback_timing`.
        assert_eq!(
            tracked_schedule_names(one, TrackedCallbackTiming::DuringCall),
            vec![
                "createComputed",
                "createMemo",
                "createRenderEffect",
                "createResource",
                // `solid-js/web`'s `effect`, which 1.x aliases to
                // `createRenderEffect` (`Solid1x::ALIASES`) — the same eager
                // primitive under a second published name.
                "effect",
                "mergeProps"
            ]
        );
        assert_eq!(
            tracked_schedule_names(one, TrackedCallbackTiming::AfterCall),
            vec!["createEffect"]
        );

        // 2.0 disagrees with 1.x on `createEffect` — `effect()` recomputes the
        // tracked compute during the call there — and its one deferring member
        // is `createTrackedEffect`, which only enqueues.
        assert_eq!(
            tracked_schedule_names(two, TrackedCallbackTiming::DuringCall),
            vec![
                "createEffect",
                "createMemo",
                "createOptimistic",
                "createProjection",
                "createRenderEffect",
                "createSignal"
            ]
        );
        assert_eq!(
            tracked_schedule_names(two, TrackedCallbackTiming::AfterCall),
            vec!["createTrackedEffect"]
        );

        // The refusals, named rather than merely absent. 1.x `createSignal`
        // never invokes its argument, `children`/`createSelector` have no
        // schedule row in contract emission, and 2.0's store pair resisted
        // measurement.
        for primitive in [
            Primitive::CreateSignal,
            Primitive::Children,
            Primitive::CreateSelector,
            Primitive::CreateDeferred,
        ] {
            assert_eq!(
                one.tracked_callback_timing(primitive, 0, 2),
                None,
                "{primitive:?}"
            );
        }
        for primitive in [
            Primitive::CreateStore,
            Primitive::CreateOptimisticStore,
            Primitive::Dynamic,
            Primitive::MapArray,
        ] {
            assert_eq!(
                two.tracked_callback_timing(primitive, 0, 2),
                None,
                "{primitive:?}"
            );
        }

        // A schedule is only ever stated for a callback this dialect calls
        // `Tracked`. `createEffect`'s second argument is 1.x's seed value and
        // 2.0's deferred apply; neither is this method's domain.
        assert_eq!(
            one.tracked_callback_timing(Primitive::CreateEffect, 1, 2),
            None
        );
        assert_eq!(
            two.tracked_callback_timing(Primitive::CreateEffect, 1, 2),
            None
        );
        assert_eq!(one.tracked_callback_timing(Primitive::Untrack, 0, 1), None);
        assert_eq!(two.tracked_callback_timing(Primitive::Untrack, 0, 1), None);
        // 1.x's `createResource(fetcher)` one-argument form has a deferred
        // fetcher at 0 and no tracked source, so the two-argument answer must
        // not leak into it.
        assert_eq!(
            one.tracked_callback_timing(Primitive::CreateResource, 0, 1),
            None
        );
        assert_eq!(
            one.tracked_callback_timing(Primitive::CreateResource, 0, 2),
            Some(TrackedCallbackTiming::DuringCall)
        );
    }

    #[test]
    fn the_async_boundary_is_a_role_not_a_name() {
        let one = Version::V1.dialect();
        let two = Version::V2.dialect();

        assert_eq!(one.boundary_kind("Suspense"), Some(Boundary::Async));
        assert_eq!(two.boundary_kind("Loading"), Some(Boundary::Async));
        assert_eq!(one.boundary_kind("ErrorBoundary"), Some(Boundary::Error));

        // Each dialect refuses the other's spelling.
        assert_eq!(one.boundary_kind("Loading"), None);
        assert_eq!(two.boundary_kind("Suspense"), None);
    }

    #[test]
    fn names_absent_from_solid_1_are_not_recognized() {
        // docs/adr — the 1.x API surface was extracted from solid-js 1.9.14 and
        // each of these was verified as 0 occurrences in the published package.
        let absent = [
            "ownedWrite",
            "onSettled",
            "createTrackedEffect",
            "createProjection",
            "createOptimistic",
            "createOptimisticStore",
            "flush",
            "affects",
            "refresh",
            "isPending",
            "Repeat",
            "Loading",
            "Reveal",
            "action",
            "createOwner",
            "merge",
            "omit",
            "deep",
            "snapshot",
            "createAsync",
        ];
        let one = Version::V1.dialect();
        for name in absent {
            assert_eq!(
                one.primitive(name),
                None,
                "{name} does not exist in Solid 1.x"
            );
        }
    }

    #[test]
    fn names_removed_in_solid_2_are_not_recognized() {
        let two = Version::V2.dialect();
        for name in ["batch", "createComputed", "createResource", "Suspense"] {
            assert_eq!(two.primitive(name), None, "{name} is gone in Solid 2.0");
        }
    }

    #[test]
    fn a_resolved_solid_js_version_picks_its_dialect() {
        assert_eq!(Version::for_solid_js("1.9.14"), Some(Version::V1));
        // 2.0 is still a prerelease; refusing to classify the RC would leave
        // every current 2.0 project on the caller's fallback.
        assert_eq!(Version::for_solid_js("2.0.0-rc.0"), Some(Version::V2));
        assert_eq!(Version::for_solid_js("^1.8.0"), Some(Version::V1));
        assert_eq!(Version::for_solid_js("v2.0.0"), Some(Version::V2));
        // No guessing: a major nobody has released and a string that is not a
        // version both answer None, so the caller decides rather than
        // inheriting a silent default from here.
        assert_eq!(Version::for_solid_js("3.0.0"), None);
        assert_eq!(Version::for_solid_js("workspace:*"), None);
        assert_eq!(Version::for_solid_js(""), None);
    }

    #[test]
    fn the_boundary_name_round_trips_through_the_boundary_kind() {
        for version in [Version::V1, Version::V2] {
            let dialect = version.dialect();
            for boundary in [Boundary::Async, Boundary::Error] {
                let name = dialect.boundary_name(boundary);
                assert_eq!(
                    dialect.boundary_kind(name),
                    Some(boundary),
                    "{name} is the {boundary:?} boundary in {version:?}"
                );
            }
        }
        assert_eq!(
            Version::V1.dialect().boundary_name(Boundary::Async),
            "Suspense"
        );
        assert_eq!(
            Version::V2.dialect().boundary_name(Boundary::Async),
            "Loading"
        );
    }

    /// Every name a dialect knows is a real export of a module it owns.
    ///
    /// This is the cross-check the vocabulary never had. The tables are
    /// hand-written from a published API surface; the index is parsed out of
    /// the installed package. A name in the first and not the second is either
    /// a typo or a name the package dropped, and either way the engine matches
    /// a call it will never see.
    #[test]
    fn every_primitive_is_exported_from_a_module_the_dialect_owns() {
        for version in [Version::V1, Version::V2] {
            let dialect = version.dialect();
            for name in dialect_names(dialect).iter().copied() {
                let modules = dialect.export_modules(name, ExportPosition::Value);
                assert!(
                    !modules.is_empty(),
                    "{version:?} has {name} in its vocabulary, and the installed package exports no such name"
                );
                for module in modules {
                    assert!(
                        dialect.owns_module(module),
                        "{name} claims {module} in {version:?}, which the dialect does not own"
                    );
                }
            }
        }

        // The headline split. In 1.x these live under a subpath; 2.0 folded
        // them into core and has no subpath to fold them out of.
        let one = Version::V1.dialect();
        assert_eq!(
            one.export_modules("createStore", ExportPosition::Value),
            ["solid-js/store"]
        );
        assert_eq!(
            one.export_modules("createSignal", ExportPosition::Value),
            ["solid-js"]
        );
        let two = Version::V2.dialect();
        assert_eq!(
            two.export_modules("createStore", ExportPosition::Value),
            ["solid-js"]
        );

        // A name exported from two modules resolves from either, and the
        // single-module predecessor could only ever name one of them.
        assert_eq!(
            one.export_modules("Show", ExportPosition::Value),
            ["solid-js", "solid-js/web"]
        );

        // Types are a position, not a separate namespace. `Store` is 1.x's
        // store type and no value at all; a value-position lookup finds
        // nothing, which is what keeps `import { Store }` -- already a
        // TypeScript error -- from being reported twice.
        assert_eq!(
            one.export_modules("Store", ExportPosition::Type),
            ["solid-js/store"]
        );
        assert!(
            one.export_modules("Store", ExportPosition::Value)
                .is_empty()
        );

        // A name the dialect does not export has no module, which is not the
        // same answer as "the package root". This is where the old fallback
        // was wrong: it answered `solid-js` for everything it had no arm for.
        assert!(
            two.export_modules("createResource", ExportPosition::Value)
                .is_empty()
        );
        assert!(
            one.export_modules("flush", ExportPosition::Value)
                .is_empty()
        );
    }

    /// The three per-argument questions are independent, and each has at least
    /// one primitive where the two dialects answer differently.
    ///
    /// Every one of these pairs was a single hardcoded list in the engine, and
    /// in every case the list held 2.0's answer.
    #[test]
    fn the_dialects_disagree_about_where_and_when_arguments_are() {
        let one = Version::V1.dialect();
        let two = Version::V2.dialect();

        // createEffect: 1.x's second argument is a seed, 2.0's is the apply
        // callback. A read in a 1.x seed was reported as running in an "apply
        // callback" 1.x does not have.
        assert_eq!(
            one.callback_executions(Primitive::CreateEffect),
            [(0, Execution::Tracked)]
        );
        assert_eq!(
            two.callback_executions(Primitive::CreateEffect),
            [(0, Execution::Tracked), (1, Execution::Deferred)]
        );

        // createMemo's options: 1.x `(fn, value?, options?)`, 2.0
        // `(compute, options?)`.
        assert_eq!(one.options_argument(Primitive::CreateMemo), Some(2));
        assert_eq!(two.options_argument(Primitive::CreateMemo), Some(1));
        assert!(!one.supports_sync_option(Primitive::CreateMemo));
        assert!(two.supports_sync_option(Primitive::CreateMemo));
        // And createStore's, the other way round.
        assert_eq!(one.options_argument(Primitive::CreateStore), Some(1));
        assert_eq!(two.options_argument(Primitive::CreateStore), Some(2));
        // The store family has an options slot but no `sync` routing: rc.0
        // rebuilds projection node options with only `loadingValue`/`name`,
        // so `sync: true` is inert on all three constructors (probed).
        assert!(!two.supports_sync_option(Primitive::CreateStore));
        assert!(!two.supports_sync_option(Primitive::CreateProjection));
        assert!(!two.supports_sync_option(Primitive::CreateOptimisticStore));

        // 1.x's tracked computations include createComputed, which 2.0 does
        // not have at all -- so no 2.0-shaped list could name it, and the read
        // after an await inside one went unreported.
        assert!(
            !one.callback_executions(Primitive::CreateComputed)
                .is_empty()
        );
        assert!(
            two.callback_executions(Primitive::CreateComputed)
                .is_empty()
        );

        // Stores: createMutable is 1.x's, the projection pair is 2.0's, and
        // createStore is the only one both have.
        assert!(one.returns_store(Primitive::CreateStore));
        assert!(two.returns_store(Primitive::CreateStore));
        assert!(one.returns_store(Primitive::CreateMutable));
        assert!(!two.returns_store(Primitive::CreateMutable));
        assert!(two.returns_store(Primitive::CreateProjection));
        assert!(!one.returns_store(Primitive::CreateProjection));
    }

    /// An options index that is also a callback position means the engine
    /// would read a function as an options object, or the reverse.
    #[test]
    fn no_primitive_takes_its_options_where_it_takes_a_callback() {
        for version in [Version::V1, Version::V2] {
            let dialect = version.dialect();
            for name in dialect_names(dialect).iter().copied() {
                let primitive = dialect.primitive(name).unwrap();
                let Some(options) = dialect.options_argument(primitive) else {
                    continue;
                };
                assert!(
                    !dialect
                        .callback_executions(primitive)
                        .iter()
                        .any(|(index, _)| *index == options),
                    "{version:?} puts {name}'s options and a callback both at {options}"
                );
            }
        }
    }

    /// The generated tables are binary-searched, so their order is load
    /// bearing. A generator that stopped sorting would not fail to compile —
    /// it would silently start missing names.
    #[test]
    fn the_generated_export_index_is_sorted() {
        for (label, table) in [
            ("1.x values", exports::solid_v1_solid_js::VALUES),
            ("1.x types", exports::solid_v1_solid_js::TYPES),
            ("2.0 values", exports::solid_v2_solid_js::VALUES),
            ("2.0 types", exports::solid_v2_solid_js::TYPES),
            ("web values", exports::solid_v2_solidjs_web::VALUES),
            ("web types", exports::solid_v2_solidjs_web::TYPES),
        ] {
            assert!(!table.is_empty(), "{label} is empty");
            assert!(
                table.windows(2).all(|pair| pair[0].0 < pair[1].0),
                "{label} is not sorted by name"
            );
            for (name, modules) in table {
                assert!(!modules.is_empty(), "{label}: {name} lists no module");
            }
        }
    }

    /// The distinction the owner analysis depends on, in both dialects: a
    /// callback that creates an owner and a callback that merely inherits one
    /// both sit at index 0, and treating them alike is what let an effect
    /// inside `untrack` at module scope go unreported.
    #[test]
    fn creating_an_owner_and_inheriting_one_are_distinguished() {
        for version in [Version::V1, Version::V2] {
            let dialect = version.dialect();
            assert_eq!(
                dialect.callback_owners(Primitive::CreateRoot),
                &[(0, CallbackOwner::Creates)],
                "createRoot creates an owner in {version:?}"
            );
            assert_eq!(
                dialect.callback_owners(Primitive::Untrack),
                &[(0, CallbackOwner::Inherits)],
                "untrack inherits the caller's owner in {version:?}"
            );
        }
        assert_eq!(
            Version::V1.dialect().callback_owners(Primitive::Children),
            &[(0, CallbackOwner::Creates)],
            "Solid 1.x children wraps its callback in createMemo"
        );
        // Unmodelled is not ownerless: a caller must not read an empty answer
        // as "creates no owner".
        assert!(
            Version::V2
                .dialect()
                .callback_owners(Primitive::Children)
                .is_empty()
        );

        // 2.0 splits the effect and runs apply unowned; 1.x has one callback
        // and index 1 is a seed value, so listing it would mark data as a
        // callback.
        assert_eq!(
            Version::V2
                .dialect()
                .callback_owners(Primitive::CreateEffect),
            &[(0, CallbackOwner::Creates), (1, CallbackOwner::None)]
        );
        assert_eq!(
            Version::V1
                .dialect()
                .callback_owners(Primitive::CreateEffect),
            &[(0, CallbackOwner::Creates)]
        );

        // Both signatures accept Owner | null. A concrete call sharpens this
        // flat answer from its first argument.
        for version in [Version::V1, Version::V2] {
            assert_eq!(
                version.dialect().callback_owners(Primitive::RunWithOwner),
                &[(1, CallbackOwner::Conditional)]
            );
        }

        // resolve(fn) wraps its thunk in createRoot, which its signature does
        // not suggest.
        assert_eq!(
            Version::V2.dialect().callback_owners(Primitive::Resolve),
            &[(0, CallbackOwner::Creates)]
        );
    }

    /// A declaration site and a namespace import ask different questions, and
    /// sharing one list made the narrower answer govern both. Every name in
    /// the vocabulary must be recognisable where it is declared, or adding it
    /// to the table accomplishes nothing.
    #[test]
    fn every_name_in_the_vocabulary_is_recognisable_at_its_declaration() {
        for version in [Version::V1, Version::V2] {
            let dialect = version.dialect();
            for name in dialect_names(dialect) {
                assert!(
                    dialect.declares_primitive(name),
                    "{name} is vocabulary in {version:?} but unrecognised where it is declared"
                );
            }
        }

        // The namespace set follows the census invariant on both dialects
        // now (see `every_modelled_export_resolves_through_its_namespace_module`
        // in each module); the `namespace-import-v2` fixture pins the
        // behavioural half of the widening.
        let two = Version::V2.dialect();
        assert!(two.declares_primitive("latest"));
        assert!(
            two.namespace_import_primitives("solid-js")
                .contains(&"latest")
        );
    }

    #[test]
    fn concrete_callback_contracts_cover_overloads_and_both_dialects() {
        let one = Version::V1.dialect();
        let two = Version::V2.dialect();

        assert_eq!(
            one.callback_execution_at(Primitive::CreateResource, 0, 1),
            Some(Execution::Deferred)
        );
        // The fetcher's owner is Conditional in both overloads: the sourced
        // form invokes it inside the resource's internal createComputed and
        // the unsourced form's initial load runs synchronously under the
        // caller's owner, but a later refetch() runs it from an arbitrary,
        // ownerless site.
        assert_eq!(
            one.callback_owner_at(Primitive::CreateResource, 0, 1),
            Some(CallbackOwner::Conditional)
        );
        assert!(one.reports_untracked_reads_at(Primitive::CreateResource, 0, 1));
        assert_eq!(
            one.callback_execution_at(Primitive::CreateResource, 0, 2),
            Some(Execution::Tracked)
        );
        assert_eq!(
            one.callback_owner_at(Primitive::CreateResource, 0, 2),
            Some(CallbackOwner::Creates)
        );
        assert!(!one.reports_untracked_reads_at(Primitive::CreateResource, 0, 2));
        assert_eq!(
            one.callback_execution_at(Primitive::CreateResource, 1, 2),
            Some(Execution::Deferred)
        );
        assert_eq!(
            one.callback_owner_at(Primitive::CreateResource, 1, 2),
            Some(CallbackOwner::Conditional)
        );
        assert!(one.reports_untracked_reads_at(Primitive::CreateResource, 1, 2));

        assert_eq!(
            one.callback_execution_at(Primitive::CreateSignal, 0, 1),
            None
        );
        assert_eq!(
            two.callback_execution_at(Primitive::CreateSignal, 0, 1),
            Some(Execution::Tracked)
        );
        assert_eq!(
            one.callback_execution_at(Primitive::CreateEffect, 1, 2),
            None
        );
        assert_eq!(
            two.callback_execution_at(Primitive::CreateEffect, 1, 2),
            Some(Execution::Deferred)
        );

        for (primitive, argument, execution) in [
            (Primitive::MapArray, 0, Execution::Tracked),
            (Primitive::MapArray, 1, Execution::Deferred),
            (Primitive::IndexArray, 0, Execution::Tracked),
            (Primitive::IndexArray, 1, Execution::Deferred),
            (Primitive::ModifyMutable, 1, Execution::Inline),
            (Primitive::CatchError, 0, Execution::Inline),
            (Primitive::CatchError, 1, Execution::Deferred),
        ] {
            assert_eq!(
                one.callback_execution_at(primitive, argument, 2),
                Some(execution),
                "missing 1.x callback contract for {primitive:?}[{argument}]"
            );
        }
        assert_eq!(
            one.callback_accessor_parameters(Primitive::MapArray, 1),
            &[1]
        );
        assert_eq!(
            one.callback_accessor_parameters(Primitive::IndexArray, 1),
            &[0]
        );
        assert!(one.reports_untracked_reads_at(Primitive::MapArray, 1, 2));
        assert!(one.reports_untracked_reads_at(Primitive::IndexArray, 1, 2));
        assert!(one.reports_untracked_reads_at(Primitive::RunWithOwner, 1, 2));
        for (primitive, argument, execution) in [
            (Primitive::Action, 0, Execution::Deferred),
            (Primitive::Flush, 0, Execution::Inline),
            (Primitive::CreateErrorBoundary, 1, Execution::Tracked),
            (Primitive::CreateLoadingBoundary, 1, Execution::Tracked),
            (Primitive::RepeatMap, 0, Execution::Tracked),
            (Primitive::RepeatMap, 1, Execution::Inline),
        ] {
            assert_eq!(
                two.callback_execution_at(primitive, argument, 2),
                Some(execution),
                "missing 2.0 callback contract for {primitive:?}[{argument}]"
            );
        }
    }

    /// Source discovery is where every read-tracing rule starts, and the two
    /// dialects create sources with different primitives. One list served both
    /// until this was a dialect question, and it was 2.0's.
    #[test]
    fn each_dialect_knows_its_own_reactive_source_factories() {
        let one = Version::V1.dialect();
        let two = Version::V2.dialect();

        for primitive in [
            Primitive::CreateSignal,
            Primitive::CreateMemo,
            Primitive::CreateStore,
        ] {
            assert!(one.creates_reactive_source(primitive));
            assert!(two.creates_reactive_source(primitive));
        }

        // 1.x-only factories. `createResource` is the one that matters most:
        // it returns a tuple, so the bundled contract's single-value `returns`
        // column cannot describe it and only this answer finds it.
        for primitive in [
            Primitive::CreateResource,
            Primitive::CreateMutable,
            Primitive::CreateDeferred,
            Primitive::CreateSelector,
        ] {
            assert!(
                one.creates_reactive_source(primitive),
                "{primitive:?} produces a reactive source in 1.x"
            );
            assert!(
                !two.creates_reactive_source(primitive),
                "{primitive:?} is not 2.0 vocabulary at all"
            );
        }

        // Returns nothing, so it is not a source however reactive it is.
        assert!(!one.creates_reactive_source(Primitive::CreateComputed));
        // 2.0-only factories.
        for primitive in [Primitive::CreateProjection, Primitive::CreateOptimistic] {
            assert!(two.creates_reactive_source(primitive));
            assert!(!one.creates_reactive_source(primitive));
        }
    }

    /// Four of the five control-flow components are shared; the fifth is the
    /// point. A function written inside one is a callback, and reading the
    /// wrong list means reading it as a component instead.
    #[test]
    fn the_fifth_control_flow_component_differs_between_dialects() {
        let one = Version::V1.dialect();
        let two = Version::V2.dialect();
        for primitive in [
            Primitive::For,
            Primitive::Show,
            Primitive::Match,
            Primitive::Switch,
        ] {
            assert!(one.renders_children_through_callback(primitive));
            assert!(two.renders_children_through_callback(primitive));
        }
        assert!(one.renders_children_through_callback(Primitive::Index));
        assert!(!two.renders_children_through_callback(Primitive::Index));
        assert!(two.renders_children_through_callback(Primitive::Repeat));
        assert!(!one.renders_children_through_callback(Primitive::Repeat));

        // Boundaries render children directly, not through a callback.
        for (dialect, boundary) in [
            (one, Primitive::Suspense),
            (one, Primitive::ErrorBoundary),
            (two, Primitive::Loading),
        ] {
            assert!(!dialect.renders_children_through_callback(boundary));
        }
    }

    #[test]
    fn conditional_cleanup_rules_survive_the_extraction() {
        let two = Version::V2.dialect();
        // Always forbidden.
        assert_eq!(two.cleanup_rule(Primitive::OnCleanup), CleanupRule::Always);
        assert_eq!(two.cleanup_rule(Primitive::Flush), CleanupRule::Always);
        assert_eq!(two.cleanup_rule(Primitive::Children), CleanupRule::Always);
        // Forbidden only when seeded with a function.
        assert_eq!(
            two.cleanup_rule(Primitive::CreateSignal),
            CleanupRule::WhenFirstArgumentIsFunction
        );
        assert_eq!(
            two.cleanup_rule(Primitive::CreateStore),
            CleanupRule::WhenFirstArgumentIsFunction
        );
        // Unrestricted.
        assert_eq!(two.cleanup_rule(Primitive::For), CleanupRule::Never);

        // createReaction allocates a computation the moment it is called, in
        // both runtimes, so it carries the same leaf-scope disposal
        // obligation as createEffect.
        for version in [Version::V1, Version::V2] {
            let dialect = version.dialect();
            assert_eq!(
                dialect.cleanup_rule(Primitive::CreateReaction),
                CleanupRule::Always,
                "createReaction needs disposal in {version:?}"
            );
            assert_eq!(
                dialect.cleanup_rule(Primitive::CreateEffect),
                CleanupRule::Always
            );
        }
    }

    #[test]
    fn module_ownership_follows_the_dialects_package_layout() {
        let one = Version::V1.dialect();
        let two = Version::V2.dialect();

        // 1.x splits stores and DOM into subpaths; importing createStore from
        // "solid-js" is wrong there and right in 2.0.
        assert!(one.owns_module("solid-js/store"));
        assert!(one.owns_module("solid-js/web"));
        assert!(!two.owns_module("solid-js/store"));
        assert!(two.owns_module("@solidjs/web"));
        assert!(one.owns_module("solid-js"));
        assert!(two.owns_module("solid-js"));
    }

    fn dialect_names(dialect: &'static dyn Dialect) -> Vec<&'static str> {
        match dialect.version() {
            Version::V1 => solid_1x::names(),
            Version::V2 => solid_2::names(),
        }
    }

    /// For every name both vocabularies resolve, the callback-shape answers
    /// the engine asks beyond `callback_executions` must agree — or the
    /// difference must be exempted here with its reason. These methods have
    /// defaults, so a missing override on one side is a silent behavioural
    /// asymmetry rather than a compile error; this test is what turns that
    /// silence into a failure.
    #[test]
    fn shared_primitive_callback_shapes_agree_or_are_exempted() {
        let one = Version::V1.dialect();
        let two = Version::V2.dialect();
        // (name, question) pairs where the runtimes genuinely differ.
        let exempted = |name: &str, question: &str| {
            matches!(
                (name, question),
                // 1.x runs map callbacks untracked (dependencies come from
                // the list argument); the 2.0 runtime tracks them — see the
                // bundled contract rows for `mapArray`.
                ("mapArray", "reports-untracked-reads")
                // The RC.0 runtime emits STRICT_READ_UNTRACKED for a
                // reactive read in the invalidation callback; the 1.x
                // runtime has no such warning and models the callback as a
                // leaf owner instead (see the `dialect-solid-*` fixture
                // pair).
                | ("createReaction", "reports-untracked-reads")
            )
        };
        for name in dialect_names(one) {
            let Some(primitive_one) = one.primitive(name) else {
                continue;
            };
            let Some(primitive_two) = two.primitive(name) else {
                continue;
            };
            for argument in 0..3 {
                for argument_count in 1..4 {
                    assert!(
                        one.reports_untracked_reads_at(primitive_one, argument, argument_count)
                            == two.reports_untracked_reads_at(
                                primitive_two,
                                argument,
                                argument_count
                            )
                            || exempted(name, "reports-untracked-reads"),
                        "{name}: reports_untracked_reads_at({argument}, {argument_count}) differs between dialects and is not exempted"
                    );
                    for result_slot in [None, Some(0), Some(1)] {
                        assert!(
                            one.returned_callback_execution_at(
                                primitive_one,
                                result_slot,
                                argument,
                                argument_count
                            ) == two.returned_callback_execution_at(
                                primitive_two,
                                result_slot,
                                argument,
                                argument_count
                            ) || exempted(name, "returned-callback-execution"),
                            "{name}: returned_callback_execution_at({result_slot:?}, {argument}, {argument_count}) differs between dialects and is not exempted"
                        );
                        assert!(
                            one.returned_callback_owner_at(
                                primitive_one,
                                result_slot,
                                argument,
                                argument_count
                            ) == two.returned_callback_owner_at(
                                primitive_two,
                                result_slot,
                                argument,
                                argument_count
                            ) || exempted(name, "returned-callback-owner"),
                            "{name}: returned_callback_owner_at({result_slot:?}, {argument}, {argument_count}) differs between dialects and is not exempted"
                        );
                    }
                }
                assert!(
                    one.callback_requires_return_invocation(primitive_one, argument)
                        == two.callback_requires_return_invocation(primitive_two, argument)
                        || exempted(name, "requires-return-invocation"),
                    "{name}: callback_requires_return_invocation({argument}) differs between dialects and is not exempted"
                );
            }
        }
    }

    #[test]
    fn owner_requirement_roles_need_unambiguous_dialect_ownership() {
        assert_eq!(
            unambiguous_owner_requirement_role("createEffect"),
            Some(OwnerRequirementRole::Effect)
        );
        assert_eq!(
            unambiguous_owner_requirement_role("onCleanup"),
            Some(OwnerRequirementRole::Cleanup)
        );
        assert_eq!(
            unambiguous_owner_requirement_role("onSettled"),
            Some(OwnerRequirementRole::SettledCleanup)
        );
        assert_eq!(unambiguous_owner_requirement_role("effect"), None);
        assert_eq!(unambiguous_owner_requirement_role("createUnknown"), None);
    }

    #[test]
    fn implementation_roles_require_canonical_cross_dialect_agreement() {
        assert!(unambiguous_callback_argument("createMemo", 0, 1));
        assert!(!unambiguous_callback_argument("createEffect", 1, 2));
        assert!(!unambiguous_callback_argument("effect", 0, 1));
        assert!(unambiguous_callable_result_tuple_item("createSignal", 0));
        assert!(unambiguous_callable_result_tuple_item("createSignal", 1));
        assert!(!unambiguous_callable_result_tuple_item("createSignal", 2));
        assert!(!unambiguous_callable_result_tuple_item("createUnknown", 0));
        assert!(unambiguous_callable_type("solid-js", "Accessor"));
        assert!(unambiguous_callable_type("solid-js", "Setter"));
        assert!(!unambiguous_callable_type("user-module", "Accessor"));
        assert!(!unambiguous_callable_type("solid-js", "Signal"));
    }
}
