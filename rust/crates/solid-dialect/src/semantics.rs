//! The closed enums and records a dialect uses to describe callbacks, owners and boundaries.

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
    /// The call site's owner on the callback's **first** run, and no owner on
    /// every later one.
    ///
    /// The shape of a callback the primitive runs once *during* the creating
    /// call, in the caller's owner and listener context, and afterwards from
    /// the scheduler's flush, where no owner is current. It is not
    /// [`Self::Inherits`], which would certify what a later run creates, and
    /// not [`Self::None`], which would claim the first run is unowned too:
    /// what the callback creates is owned exactly as the call site is on its
    /// first run and may be detached on a later one.
    ///
    /// Write legality follows the same split. A write on the first run
    /// answers to the call site's owner; a write on a later run is not under
    /// any owner.
    InheritsFirstRun,
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
/// These three analyzer roles are projections of independent tracking,
/// event, and schedule axes. The checked export authorities hold the tables
/// below to the exact published package behavior.
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
// **Callback summaries are the second consumer, and they do ask when the
// callback ran.** `callback_wrapper_at` (solid-reactive-ir/src/interproc.rs)
// reads these same rows to compose an `execution` word for an export, and
// that word is a statement about the clock: `inline` states the export
// invoked the callback before returning, `deferred` states it did not. So a
// summary never reads the schedule *out of* this word. It reads the word for
// attribution and takes the schedule from separate dialect facts —
// [`Dialect::runs_callback_synchronously`] for the listener-clearing
// primitives that nonetheless run during the call, and
// [`Dialect::tracked_callback_timing`] for when a tracked computation runs
// relative to the call that creates it. Where a dialect states no schedule
// fact, the summary records no word rather than reading one off this one.
//
// The two divergences above are exactly where the readings differ, and both are
// closed by that split rather than papered over: `startTransition` and
// `createResource` are absent from `primitive_callback_execution`'s schedule
// table, so a summary refuses them instead of restating their attribution as
// a schedule.

/// When a primitive's [`Execution::Tracked`] callback runs, relative to the
/// primitive's own call returning.
///
/// Orthogonal to [`Execution`], which says who owns the reads. A tracked
/// computation is tracked either way; this says whether the export that created
/// it has already run it by the time it returns, which is the only thing a
/// summary `execution` word can state about a callback the package
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

/// What the dev owned-scope write guards (signal and store setters, `refresh`)
/// do while the runtime disposes an owner, on the resolved release.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WriteGuardInDisposal {
    /// The guard still throws there: a read release before rc.17.
    Rejects,
    /// The guard tests `!disposalDepth` and stands down: rc.17.
    StandsDown,
    /// No release that was read answers: an installation whose signals nobody
    /// read or that did not resolve, and the conservative vocabulary. Outside
    /// a disposal the guard throws on every release, so such a write is still
    /// reported there; one a disposal decides is uncertifiable, never legal.
    #[default]
    Unknown,
}

/// Invocation trigger, not an execution-role or ownership assertion. Except
/// DuringCall, every variant needs a separate feasible-delivery premise.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BrowserCallbackTrigger {
    DuringCall,
    ComputationDemand,
    EffectFlush,
    ComputationError,
    OwnerDisposal,
    Settlement,
    RenderSelection,
    DomEvent,
    TimerTask,
    AnimationFrame,
    IdleTask,
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

/// A positively identified runtime-configuration export (ADRs 0266/0268).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeConfigurationApi {
    ExternalSource,
    DevelopmentHooks,
    Observation,
    HydrationHost,
}

/// Candidate host booleans. This vocabulary is not authority: the host
/// consumer must resolve the exact declaration and authenticate every runtime
/// target under both hosts before supplying the values to syntax predicates.
pub struct HostBooleanExport {
    pub module: &'static str,
    pub export: &'static str,
    pub version: &'static str,
    pub client: bool,
    pub server: bool,
    pub runtime_targets: &'static [&'static str],
}
