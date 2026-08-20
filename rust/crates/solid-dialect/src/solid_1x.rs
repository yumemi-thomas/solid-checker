//! Solid 1.x.
//!
//! Every name here comes from `docs/solid-1x-api-surface.md`, which was
//! extracted from the published `solid-js@1.9.14` package rather than from
//! documentation or memory. That file states the rule this module follows:
//! **do not add a name to the vocabulary that is not on that list.** If
//! something looks missing, re-extract from the package first.
//!
//! The vocabulary is deliberately narrower than that list, and narrower than
//! the bundled 1.x contract's 144 exports: a name earns a place here when the
//! checker models a reactive obligation for it. `createUniqueId` is a real 1.x
//! export with nothing to say about reactivity, and it is not here.

use crate::{
    Boundary, CallbackOwner, CleanupRule, Dialect, Execution, Primitive, Version, lookup, reverse,
};

/// Solid 1.x.
#[derive(Clone, Copy, Debug, Default)]
pub struct Solid1x;

/// Source: `docs/solid-1x-api-surface.md`, sections `solid-js`,
/// `solid-js/store` and the control-flow components. Names the checker does
/// not model — `createUniqueId`, `observable`, the SSR helpers — are omitted
/// deliberately; they carry no reactive obligation.
const TABLE: &[(&str, Primitive)] = &[
    ("batch", Primitive::Batch),
    ("catchError", Primitive::CatchError),
    ("children", Primitive::Children),
    ("createComputed", Primitive::CreateComputed),
    ("createContext", Primitive::CreateContext),
    ("createDeferred", Primitive::CreateDeferred),
    ("createDynamic", Primitive::CreateDynamic),
    ("createEffect", Primitive::CreateEffect),
    ("createMemo", Primitive::CreateMemo),
    ("createMutable", Primitive::CreateMutable),
    ("createReaction", Primitive::CreateReaction),
    ("createRenderEffect", Primitive::CreateRenderEffect),
    ("createResource", Primitive::CreateResource),
    ("createRoot", Primitive::CreateRoot),
    ("createSelector", Primitive::CreateSelector),
    ("createSignal", Primitive::CreateSignal),
    ("createStore", Primitive::CreateStore),
    ("ErrorBoundary", Primitive::ErrorBoundary),
    ("For", Primitive::For),
    ("from", Primitive::From),
    ("getOwner", Primitive::GetOwner),
    ("hydrate", Primitive::Hydrate),
    ("Index", Primitive::Index),
    ("indexArray", Primitive::IndexArray),
    ("lazy", Primitive::Lazy),
    ("mapArray", Primitive::MapArray),
    ("Match", Primitive::Match),
    ("memo", Primitive::WebMemo),
    ("mergeProps", Primitive::MergeProps),
    ("modifyMutable", Primitive::ModifyMutable),
    ("on", Primitive::On),
    ("onCleanup", Primitive::OnCleanup),
    ("onError", Primitive::OnError),
    ("onMount", Primitive::OnMount),
    ("produce", Primitive::Produce),
    ("reconcile", Primitive::Reconcile),
    ("render", Primitive::Render),
    ("runWithOwner", Primitive::RunWithOwner),
    ("Show", Primitive::Show),
    ("splitProps", Primitive::SplitProps),
    ("startTransition", Primitive::StartTransition),
    ("Suspense", Primitive::Suspense),
    ("SuspenseList", Primitive::SuspenseList),
    ("Switch", Primitive::Switch),
    ("untrack", Primitive::Untrack),
    ("unwrap", Primitive::Unwrap),
    ("useContext", Primitive::UseContext),
    ("useTransition", Primitive::UseTransition),
];

/// Public aliases whose implementation is the canonical primitive itself.
/// The web entrypoint re-exports `createRenderEffect as effect`, so losing the
/// alias also loses the effect's owner and disposal obligations.
const ALIASES: &[(&str, Primitive)] = &[("effect", Primitive::CreateRenderEffect)];

/// Every name this dialect exports, derived from [`TABLE`] rather than
/// mirrored beside it. The mirror was a second list to keep in step, and
/// keeping two lists in step by hand is the defect this crate exists to
/// remove one level down.
#[cfg(test)]
pub(crate) fn names() -> Vec<&'static str> {
    TABLE.iter().chain(ALIASES).map(|(name, _)| *name).collect()
}

impl Dialect for Solid1x {
    fn version(&self) -> Version {
        Version::V1
    }

    fn component_name_may_be_component(&self, name: &str) -> bool {
        name.as_bytes()
            .first()
            .is_some_and(|byte| !byte.is_ascii_lowercase())
    }

    /// 1.x ships four user-facing subpaths, and the distinction is load
    /// bearing: `createStore` imported from `solid-js` is wrong in 1.x and
    /// correct in 2.0.
    fn modules(&self) -> &'static [&'static str] {
        &[
            "solid-js",
            "solid-js/store",
            "solid-js/web",
            "solid-js/universal",
        ]
    }

    /// `pkg/contracts/bundled/solid-v1/solid-js.json`, the artifact
    /// `solid-facts-backend` compiles in for this dialect — not the reviewed
    /// semantics source (`contracts/solid-v1/solid-js.json` in this crate), which
    /// no diagnostic reads at runtime.
    fn bundled_contract_label(&self) -> &'static str {
        "solid-v1/solid-js.json"
    }

    /// The 1.x dom-expressions `reservedNameSpaces` (`class`, `on`,
    /// `oncapture`, `style`, `use`, `prop`, `attr`, `bool`) plus the XML
    /// prefixes the compiler maps (`xmlns`, `xlink`, `xml` — dom-expressions
    /// 0.40's namespace table, so `xml:lang` and `xml:space` stay legal SVG).
    /// `class:` and `style:` are recognized — the compiler binds them
    /// per-name — even though `no-unknown-namespaces` still steers authors
    /// toward the plain props, exactly as upstream's rule does.
    fn jsx_attribute_namespaces(&self) -> &'static [&'static str] {
        &[
            "class",
            "on",
            "oncapture",
            "style",
            "use",
            "prop",
            "attr",
            "bool",
            "xmlns",
            "xlink",
            "xml",
        ]
    }

    fn primitive(&self, name: &str) -> Option<Primitive> {
        static INDEX: crate::NameIndex = crate::NameIndex::new();
        lookup(&INDEX, &[TABLE, ALIASES], name)
    }

    fn name_of(&self, primitive: Primitive) -> Option<&'static str> {
        reverse(TABLE, primitive)
    }

    /// Source: the signatures quoted in `docs/solid-1x-api-surface.md`.
    ///
    /// `createEffect<Next, Init>(fn, value?, options?)` — the callback is at
    /// index 0 and **index 1 is a seed value, not a callback**. This is the
    /// difference ADR 0001 led with, inverted: reading 1.x's seed as 2.0's
    /// apply callback is the highest-yield mistake available here.
    ///
    /// The engine once made this exact mistake (a pre-vocabulary site read
    /// the seed value as the callback); wiring it onto this table fixed
    /// that, and this table is now the only place the answer lives.
    fn callback_positions(&self, primitive: Primitive) -> &'static [usize] {
        match primitive {
            Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateComputed
            | Primitive::CreateMemo
            | Primitive::CreateDeferred
            | Primitive::CreateDynamic
            | Primitive::CreateReaction
            | Primitive::CreateRoot
            | Primitive::Children
            | Primitive::Untrack
            | Primitive::Batch
            | Primitive::StartTransition
            | Primitive::OnMount
            | Primitive::OnCleanup
            | Primitive::OnError
            | Primitive::From
            | Primitive::Hydrate
            | Primitive::Lazy
            | Primitive::Render
            | Primitive::WebMemo
            | Primitive::Produce => &[0],
            Primitive::CreateSelector => &[0, 1],
            Primitive::CatchError | Primitive::On => &[0, 1],
            // createResource(source, fetcher) — the fetcher may sit at either
            // index depending on whether a source is supplied.
            Primitive::CreateResource => &[0, 1],
            // mapArray(list, mapFn) / indexArray(list, mapFn): the returned
            // accessor calls the list under tracking and the mapper under
            // `untrack`.
            Primitive::MapArray | Primitive::IndexArray => &[0, 1],
            // runWithOwner(owner, fn) / modifyMutable(state, modifier)
            Primitive::RunWithOwner | Primitive::ModifyMutable => &[1],
            _ => &[],
        }
    }

    /// Callbacks that explicitly clear tracking or run later outside their
    /// creating computation. Synchronous wrappers such as `batch`, `from`,
    /// and the protected body of `catchError` are absent because the 1.9.14
    /// runtime preserves `Listener` while invoking them.
    ///
    /// These roles are pinned to the published `solid-js@1.9.14` runtime and
    /// checked against the bundled package contract below. In particular,
    /// `lazy(loader)` stores `loader` on the returned component and invokes it
    /// only when that component (or its `preload` method) is called.
    fn runs_callback_deferred(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::Untrack
                | Primitive::CreateRoot
                | Primitive::CreateReaction
                | Primitive::OnMount
                | Primitive::OnCleanup
                | Primitive::OnError
                | Primitive::Lazy
                | Primitive::RunWithOwner
        )
    }

    /// 1.x's async boundary is `Suspense`; its error boundary is
    /// `ErrorBoundary`. There is no `Loading` and no `Errored` —
    /// `docs/solid-1x-api-surface.md` records `Errored` as a false positive,
    /// present only as the `"errored"` member of `createResource`'s state
    /// union.
    ///
    /// `SuspenseList` is deliberately absent. In the published 1.9.14 runtime
    /// only `Suspense` provides a `SuspenseContext`; `SuspenseList` registers
    /// a `SuspenseListContext` that coordinates the *reveal order* of child
    /// `Suspense` boundaries and bounds nothing itself. A pending read beneath
    /// a `SuspenseList` with no `Suspense` between them escapes to the nearest
    /// outer `Suspense`, exactly as if the list were not there.
    fn boundary_kind(&self, tag: &str) -> Option<Boundary> {
        match tag {
            "Suspense" => Some(Boundary::Async),
            "ErrorBoundary" => Some(Boundary::Error),
            _ => None,
        }
    }

    /// The 1.x shape of the same question `Solid2::callback_owners` answers.
    ///
    /// Two differences from 2.0, both consequences of the effect split not
    /// having happened yet:
    ///
    /// - `createEffect(fn, value?)` has one callback and it is owned. There is
    ///   no apply phase to run unowned, so unlike 2.0 there is no index 1
    ///   entry -- reading 1.x's seed value as 2.0's apply is the mistake
    ///   [`Solid1x::callback_positions`] exists to prevent, and repeating it
    ///   here would mark a seed value as an unowned callback.
    /// - `batch` and `startTransition` join `untrack` as callbacks that
    ///   inherit: they change how updates are scheduled, not who owns them.
    ///
    /// The published 1.9.14 runtime is the evidence for these owner roles.
    /// Higher-order helpers still require a call-site proof before their edge
    /// exists; an owner role never implies that a dormant callback ran.
    fn callback_owners(&self, primitive: Primitive) -> &'static [(usize, CallbackOwner)] {
        match primitive {
            Primitive::CreateRoot
            | Primitive::Children
            | Primitive::CreateMemo
            | Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateComputed
            | Primitive::CreateDeferred
            | Primitive::CreateDynamic => &[(0, CallbackOwner::Creates)],
            Primitive::CreateSelector => {
                &[(0, CallbackOwner::Creates), (1, CallbackOwner::Inherits)]
            }
            Primitive::WebMemo => &[(0, CallbackOwner::Creates)],
            Primitive::Hydrate | Primitive::Render => &[(0, CallbackOwner::Creates)],
            // catchError allocates a computation before invoking its protected
            // body. Its handler is deliberately absent: an immediate throw
            // runs it under that owner, while a queued reactive error runs it
            // under a synthetic error effect, so one flat owner role would
            // fabricate certainty.
            Primitive::CatchError => &[(0, CallbackOwner::Creates)],
            // The supplied owner is nullable. The call-site classifier
            // sharpens this to Creates or None when its value is proven.
            Primitive::RunWithOwner => &[(1, CallbackOwner::Conditional)],
            // The reaction owns its invalidation callback: runComputation
            // installs the reaction node as Owner, and cleanNode disposes the
            // callback's cleanups and children before the next run.
            Primitive::CreateReaction => &[(0, CallbackOwner::Creates)],
            // The flat package-contract form records the two-argument
            // fetcher. `callback_owner_at` supplies both overloads and the
            // tracked source's created owner; see it for why the fetcher's
            // owner is Conditional rather than None.
            Primitive::CreateResource => &[(1, CallbackOwner::Conditional)],
            Primitive::MapArray | Primitive::IndexArray => &[(1, CallbackOwner::Creates)],
            Primitive::ModifyMutable => &[(1, CallbackOwner::Inherits)],
            Primitive::Untrack
            | Primitive::Batch
            | Primitive::StartTransition
            | Primitive::From
            | Primitive::Lazy
            | Primitive::Produce => &[(0, CallbackOwner::Inherits)],
            Primitive::On => &[(0, CallbackOwner::Inherits), (1, CallbackOwner::Inherits)],
            _ => &[],
        }
    }

    /// `createResource`'s fetcher owner depends on which path invokes it,
    /// which is why both overloads answer [`CallbackOwner::Conditional`]
    /// rather than [`CallbackOwner::None`]. The published 1.9.14 runtime has
    /// three invocation paths:
    ///
    /// - **Sourced** `createResource(source, fetcher)` calls `load` from
    ///   inside the resource's internal `createComputed(() => load(false))`,
    ///   so the fetcher runs under that computation's owner.
    /// - **Unsourced** `createResource(fetcher)` runs its initial `load(false)`
    ///   synchronously during the `createResource` call itself, under
    ///   whatever owner the caller has.
    /// - **`refetch()`** may be called from any site — an event handler, a
    ///   timeout — and there `load` runs with no owner at all.
    ///
    /// So the fetcher usually runs owned and sometimes does not; only a
    /// call-site proof (this is the initial load, not a refetch) could
    /// sharpen the answer, and this table cannot carry one.
    fn callback_owner_at(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> Option<CallbackOwner> {
        // Every function-valued mergeProps source is wrapped in createMemo.
        // This is variadic and therefore cannot be represented by the static
        // callback_owners table's finite argument indices.
        if primitive == Primitive::MergeProps && argument < argument_count {
            return Some(CallbackOwner::Creates);
        }
        if primitive == Primitive::CreateResource {
            return match (argument_count, argument) {
                (1, 0) => Some(CallbackOwner::Conditional),
                (2.., 0) => Some(CallbackOwner::Creates),
                (2.., 1) => Some(CallbackOwner::Conditional),
                _ => None,
            };
        }
        self.callback_owners(primitive)
            .iter()
            .find(|(index, _)| *index == argument)
            .map(|(_, owner)| *owner)
    }

    /// Source: `solid-js@1.9.14`'s `types/render/flow.d.ts`, read from the
    /// installed package.
    ///
    /// ```text
    /// For<T>(props):   children: (item: T[number],           index: Accessor<number>) => U
    /// Index<T>(props): children: (item: Accessor<T[number]>, index: number)           => U
    /// ```
    ///
    /// Exact mirrors, and the engine knew only the first. `<Index>`'s item
    /// accessor was registered as a source nowhere, so a read of it was traced
    /// to nothing and reported by no rule.
    ///
    /// 1.x's `<For>` has no `keyed` prop — the three-way form is 2.0's — so
    /// the key shape does not change its answer.
    fn children_accessor_parameters(
        &self,
        primitive: Primitive,
        key: crate::KeyForm,
    ) -> &'static [usize] {
        match primitive {
            // Keyed <Show>/<Match> hand the callback the raw value; unkeyed
            // hand it an accessor. Both overloads are in flow.d.ts.
            //
            // `keyed` is a *boolean* prop in 1.x — the key-function form is
            // 2.0's — so `keyed={expr}` is a flag whose truthiness picks the
            // overload at runtime: truthy hands the callback the raw value,
            // falsy an accessor. A static table cannot prove which, and
            // claiming an accessor for a raw value would fabricate a source,
            // so both expression forms (proven-function and dynamic flag)
            // claim nothing.
            Primitive::Show | Primitive::Match => match key {
                crate::KeyForm::Keyed | crate::KeyForm::CustomKey | crate::KeyForm::DynamicFlag => {
                    &[]
                }
                crate::KeyForm::Unkeyed | crate::KeyForm::Absent => &[0],
            },
            Primitive::For => &[1],
            Primitive::Index => &[0],
            _ => &[],
        }
    }

    fn callback_accessor_parameters(
        &self,
        primitive: Primitive,
        argument: usize,
    ) -> &'static [usize] {
        match (primitive, argument) {
            (Primitive::MapArray, 1) => &[1],
            (Primitive::IndexArray, 1) => &[0],
            _ => &[],
        }
    }

    /// 1.x has two, and neither is one of 2.0's extras: `createStore` returns
    /// `[store, setStore]` and `createMutable` returns the store itself.
    /// `createProjection` and `createOptimisticStore` do not exist here, so
    /// the 2.0-shaped list the engine used was inert rather than wrong — but
    /// only by accident, and it named nothing 1.x-specific.
    fn returns_store(&self, primitive: Primitive) -> bool {
        matches!(primitive, Primitive::CreateStore | Primitive::CreateMutable)
    }

    /// Source: the declarations in `solid-js@1.9.14`, read from the installed
    /// package — `types/reactive/signal.d.ts` and `store/types/`.
    ///
    /// Every position 1.x shares with 2.0 is a coincidence, and three are not
    /// shared at all. `createMemo(fn, value?, options?)` puts options at 2
    /// where 2.0 puts them at 1; `createStore(store?, options?)` and
    /// `createMutable(state, options?)` put them at 1 where 2.0's derived
    /// forms put them at 2. The engine used 2.0's numbers for both dialects,
    /// so a 1.x `createMemo(fn, seed)` had its *seed* read as an options
    /// object — and the dialect fixture pair carried the resulting finding.
    ///
    /// `createComputed`, `createDeferred` and `createSelector` are here and
    /// were in no list before: 2.0 has none of them, so nothing 2.0-shaped
    /// could have had an opinion.
    ///
    /// `on(deps, fn, options?)` takes `{ defer }` at index 2. The slot is
    /// modelled because it is where the options are; the `defer` key itself
    /// is deliberately unmodelled. The engine's only options consumer today
    /// is sync-option detection, which every 1.x primitive opts out of via
    /// `supports_sync_option`, and `defer` is not a sync option — it makes
    /// the runtime skip `fn` until deps first *change*. Modelling that skip
    /// (the callback is dormant until a dependency writes) would need engine
    /// machinery for first-run reachability that does not exist yet; future
    /// work, not a table entry.
    fn options_argument(&self, primitive: Primitive) -> Option<usize> {
        match primitive {
            Primitive::CreateSignal
            | Primitive::CreateStore
            | Primitive::CreateMutable
            | Primitive::CreateDeferred => Some(1),
            Primitive::CreateMemo
            | Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateComputed
            | Primitive::CreateSelector
            | Primitive::On => Some(2),
            _ => None,
        }
    }

    /// Source: the reviewed `SOLID_1X` semantics table in
    /// `solid-contract-gen`, held to it by
    /// `the_callback_executions_agree_with_the_bundled_contract`.
    ///
    /// The entry that matters most is `createEffect`: **one** tracked callback,
    /// at index 0. 1.x's second argument is a seed value threaded to the next
    /// run as `prev`, not 2.0's apply callback, and the engine described reads
    /// in it as being in an "apply callback" for as long as that pair was
    /// hardcoded.
    ///
    /// `createSignal` and `createStore` are absent, unlike 2.0's: 1.x has no
    /// derived form of either, so a function passed to them is a value.
    fn callback_executions(&self, primitive: Primitive) -> &'static [(usize, Execution)] {
        match primitive {
            Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateComputed
            | Primitive::CreateMemo
            | Primitive::Children
            | Primitive::CreateDynamic
            | Primitive::CreateDeferred
            | Primitive::WebMemo => &[(0, Execution::Tracked)],
            Primitive::CreateSelector => &[(0, Execution::Tracked), (1, Execution::Inline)],
            Primitive::CreateReaction
            | Primitive::Lazy
            | Primitive::OnCleanup
            | Primitive::OnMount
            | Primitive::OnError => &[(0, Execution::Deferred)],
            // createResource(source, fetcher): the fetcher runs outside the
            // creating computation, and its index depends on whether a source
            // was supplied.
            Primitive::CreateResource => &[(1, Execution::Deferred)],
            Primitive::CatchError => &[(0, Execution::Inline), (1, Execution::Deferred)],
            Primitive::CreateRoot
            | Primitive::Untrack
            | Primitive::Batch
            // With transitions enabled the 1.9 runtime invokes
            // startTransition's callback in a Promise.resolve().then()
            // microtask, not synchronously. `Inline` is the right
            // classification anyway: [`Execution`] classifies attribution, and
            // the runtime restores the captured Listener around the callback,
            // so reads inside it subscribe exactly as at the call site and it
            // never re-runs. Probed in
            // scripts/contract-probes-solid-v1-core.mjs.
            | Primitive::StartTransition
            | Primitive::From
            | Primitive::Hydrate
            | Primitive::Render
            | Primitive::Produce => &[(0, Execution::Inline)],
            Primitive::MapArray | Primitive::IndexArray => {
                &[(0, Execution::Tracked), (1, Execution::Deferred)]
            }
            // `on`'s deps run *inline* in whatever computation invokes the
            // returned adapter — deliberately not `Tracked`, although the
            // reads do subscribe there: the engine's returned-adapter
            // classifier keys on `Inline` to derive the role from the
            // concrete invocation site (`createEffect(on(...))` tracks, a
            // bare top-level adapter call does not). `mapArray`'s list stays
            // `Tracked` because its row computations re-read it themselves.
            Primitive::On => &[(0, Execution::Inline), (1, Execution::Deferred)],
            Primitive::RunWithOwner | Primitive::ModifyMutable => &[(1, Execution::Inline)],
            _ => &[],
        }
    }

    fn callback_requires_return_invocation(&self, primitive: Primitive, argument: usize) -> bool {
        (primitive == Primitive::CreateSelector && argument == 1)
            || (argument == 0
                && matches!(
                    primitive,
                    Primitive::CreateReaction | Primitive::Lazy | Primitive::Produce
                ))
            || (argument <= 1
                && matches!(
                    primitive,
                    Primitive::MapArray | Primitive::IndexArray | Primitive::On
                ))
    }

    /// The function arguments 1.x stores instead of invoking: a signal's
    /// initial value, and the `prev` seed threaded into effects, computeds,
    /// and memos. 2.0 answers differently for the first — `createSignal(fn)`
    /// there is a derived signal whose compute runs tracked. Source:
    /// `docs/solid-1x-api-surface.md`.
    fn stores_function_argument_as_value(&self, primitive: Primitive, argument: usize) -> bool {
        (primitive == Primitive::CreateSignal && argument == 0)
            || (argument == 1
                && matches!(
                    primitive,
                    Primitive::CreateEffect
                        | Primitive::CreateRenderEffect
                        | Primitive::CreateComputed
                        | Primitive::CreateMemo
                ))
    }

    fn returned_callback_execution_at(
        &self,
        primitive: Primitive,
        result_slot: Option<usize>,
        argument: usize,
        argument_count: usize,
    ) -> Option<Execution> {
        match (primitive, result_slot, argument, argument_count) {
            (Primitive::CreateReaction, None, 0, 1..) => Some(Execution::Tracked),
            // The transition starter shares startTransition's timing caveat:
            // the 1.9 runtime may invoke it in a Promise.resolve().then()
            // microtask, but it restores the captured Listener, so
            // tracking-wise it is the call site's scope — Inline.
            (Primitive::UseTransition, Some(1), 0, 1..) => Some(Execution::Inline),
            _ => None,
        }
    }

    fn returned_callback_owner_at(
        &self,
        primitive: Primitive,
        result_slot: Option<usize>,
        argument: usize,
        argument_count: usize,
    ) -> Option<CallbackOwner> {
        match (primitive, result_slot, argument, argument_count) {
            (Primitive::CreateReaction, None, 0, 1..) => Some(CallbackOwner::Creates),
            (Primitive::UseTransition, Some(1), 0, 1..) => Some(CallbackOwner::Inherits),
            _ => None,
        }
    }

    fn callback_execution_at(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> Option<Execution> {
        // `mergeProps(...sources)` checks every source and wraps each function
        // in `createMemo`. Keep this call-shape fact native: schema v1 package
        // contracts only support fixed callback parameters, not variadics.
        if primitive == Primitive::MergeProps && argument < argument_count {
            return Some(Execution::Tracked);
        }
        if primitive == Primitive::CreateResource {
            return match (argument_count, argument) {
                (1, 0) => Some(Execution::Deferred),
                (2.., 0) => Some(Execution::Tracked),
                (2.., 1) => Some(Execution::Deferred),
                _ => None,
            };
        }
        self.callback_executions(primitive)
            .iter()
            .find(|(index, _)| *index == argument)
            .map(|(_, execution)| *execution)
    }

    fn reports_untracked_reads_at(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> bool {
        (matches!(
            primitive,
            Primitive::Hydrate | Primitive::Lazy | Primitive::Render
        ) && argument == 0)
            || (primitive == Primitive::CreateResource
                && matches!((argument_count, argument), (1, 0) | (2.., 1)))
            || (matches!(
                primitive,
                Primitive::MapArray | Primitive::IndexArray | Primitive::RunWithOwner
            ) && argument == 1)
    }

    /// `<Ctx.Provider value={...}>` — the value getter runs untracked.
    fn context_provider_member(&self) -> Option<&'static str> {
        Some("Provider")
    }

    /// The `v1/` catalog carries the SC8xxx ESLint-era surface.
    fn carries_eslint_era_rules(&self) -> bool {
        true
    }

    /// `v1/no-async-tracked-scope`; 2.0 models async computations as a
    /// feature and omits the rule.
    fn reports_async_tracked_scope(&self) -> bool {
        true
    }

    fn static_event_values_are_attributes(&self) -> bool {
        true
    }

    /// `Suspense`, not `Loading`; `ErrorBoundary`, not `Errored`. Source:
    /// `docs/solid-1x-api-surface.md`, the control-flow components.
    ///
    /// A total inverse of [`Solid1x::boundary_kind`]: `SuspenseList` is in
    /// neither, because it coordinates child `Suspense` boundaries' reveal
    /// order without providing a `SuspenseContext` of its own.
    fn boundary_name(&self, boundary: Boundary) -> &'static str {
        match boundary {
            Boundary::Async => "Suspense",
            Boundary::Error => "ErrorBoundary",
        }
    }

    /// The 2.0 rule minus the primitives 1.x does not have, plus 1.x's own
    /// owner-creating primitives. State factories are absent: a function
    /// passed to `createSignal`, `createStore`, or `createMutable` is stored as
    /// data in 1.x and registers no work in the surrounding owner.
    fn cleanup_rule(&self, primitive: Primitive) -> CleanupRule {
        match primitive {
            Primitive::OnCleanup
            | Primitive::OnMount
            | Primitive::CreateMemo
            | Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateComputed
            | Primitive::CreateDeferred
            | Primitive::CreateDynamic
            | Primitive::CreateSelector
            // createReaction allocates a computation the moment it is called
            // (1.9.14 builds one with `createComputation` and reuses it for
            // every `track`), so like createEffect it needs a surrounding
            // owner to dispose the reaction itself. Its invalidation callback
            // separately runs under the reaction's own created owner.
            | Primitive::CreateReaction
            | Primitive::CreateRoot
            | Primitive::MapArray
            | Primitive::IndexArray
            | Primitive::From
            | Primitive::Hydrate
            | Primitive::Render
            | Primitive::WebMemo
            // createResource eagerly creates computations (a render effect
            // when a source is supplied) that need disposal by their
            // surrounding owner.
            | Primitive::CreateResource
            | Primitive::Children => CleanupRule::Always,
            _ => CleanupRule::Never,
        }
    }

    /// Nothing. Returning a cleanup is a 2.0 idea.
    ///
    /// 1.x declares its effect callbacks as
    /// `EffectFunction<Prev, Next> = (v: Prev) => Next` and threads the return
    /// value to the next run, so `createEffect(prev => prev + 1, 0)` is
    /// idiomatic accumulation. `createReaction`'s `onInvalidate` and
    /// `onMount`'s callback are both declared `() => void`. Cleanup in 1.x is
    /// `onCleanup` and only `onCleanup`.
    ///
    /// The shared list this replaced named `createEffect`,
    /// `createRenderEffect` and `createReaction` — all three real in 1.x — so
    /// the accumulating form was reported as an unprovable cleanup return.
    fn accepts_cleanup_return(&self, _primitive: Primitive) -> bool {
        false
    }

    /// 1.x renders `<Index each>{item => ...}</Index>` the same way 2.0
    /// renders `<Repeat>`. The shared list the engine used named `Repeat`,
    /// which 1.x does not have, and omitted `Index`, which it does — so a
    /// function written inside an `<Index>` was read as a component.
    fn renders_children_through_callback(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::For
                | Primitive::Index
                | Primitive::Show
                | Primitive::Match
                | Primitive::Switch
        )
    }

    /// 1.x's own set, and the reason this is a dialect question at all.
    /// `createResource` returns `[accessor, { mutate, refetch }]`,
    /// `useTransition` returns `[pendingAccessor, startTransition]`, and
    /// `createMutable` returns a store. The two tuple shapes do not fit the
    /// bundled contract's single-value `returns` column — so native runtime
    /// facts must identify their reactive slots.
    ///
    /// `createComputed` is absent on purpose: it returns nothing.
    fn creates_reactive_source(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateSignal
                | Primitive::Children
                | Primitive::CreateMemo
                | Primitive::CreateDynamic
                | Primitive::CreateStore
                | Primitive::CreateMutable
                | Primitive::CreateResource
                | Primitive::CreateDeferred
                | Primitive::CreateSelector
                | Primitive::From
                | Primitive::WebMemo
                | Primitive::UseTransition
        )
    }

    fn creates_directive_owner(&self, _primitive: Primitive) -> bool {
        // Solid 1.x applies directives and refs through untrack(), which
        // clears Listener but preserves Owner. Nothing created there enters
        // the unowned directive-leak class modeled by SC6001 in Solid 2.0.
        false
    }

    /// Source: `docs/solid-1x-api-surface.md`, the `solid-js` and
    /// `solid-js/store` sections. The split is the difference that makes
    /// module gating worth having: every name below sits in core in 2.0.
    /// Straight out of the generated index: 1.x ships one package with four
    /// subpaths, so one table answers for all of them.
    fn export_modules(&self, name: &str, position: crate::ExportPosition) -> Vec<&'static str> {
        crate::exports::modules(
            crate::exports::solid_v1_solid_js::VALUES,
            crate::exports::solid_v1_solid_js::TYPES,
            name,
            position,
        )
    }

    /// Per-subpath, which is the whole reason this is module-scoped:
    /// `solid-js/store` exposes the store primitives and core does not.
    ///
    /// The checked-in package export census is the source of truth. The unit
    /// test below derives the expected names from that census and [`TABLE`],
    /// so adding a modelled obligation without its namespace spelling fails.
    fn namespace_import_primitives(&self, module: &str) -> &'static [&'static str] {
        match module {
            "solid-js" => NAMESPACE_CORE,
            "solid-js/store" => NAMESPACE_STORE,
            "solid-js/web" => NAMESPACE_WEB,
            _ => &[],
        }
    }
}

const NAMESPACE_CORE: &[&str] = &[
    "ErrorBoundary",
    "For",
    "Index",
    "Match",
    "Show",
    "Suspense",
    "SuspenseList",
    "Switch",
    "batch",
    "catchError",
    "children",
    "createComputed",
    "createContext",
    "createDeferred",
    "createEffect",
    "createMemo",
    "createReaction",
    "createRenderEffect",
    "createResource",
    "createRoot",
    "createSelector",
    "createSignal",
    "from",
    "getOwner",
    "indexArray",
    "lazy",
    "mapArray",
    "mergeProps",
    "on",
    "onCleanup",
    "onError",
    "onMount",
    "runWithOwner",
    "splitProps",
    "startTransition",
    "untrack",
    "useContext",
    "useTransition",
];

const NAMESPACE_STORE: &[&str] = &[
    "createStore",
    "produce",
    "reconcile",
    "unwrap",
    "createMutable",
    "modifyMutable",
];

const NAMESPACE_WEB: &[&str] = &[
    "ErrorBoundary",
    "For",
    "Index",
    "Match",
    "Show",
    "Suspense",
    "SuspenseList",
    "Switch",
    "createDynamic",
    "effect",
    "getOwner",
    "hydrate",
    "memo",
    "mergeProps",
    "render",
    "untrack",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_names_signal_possible_components_without_proving_them() {
        for component in ["App", "_App", "$App", "画面"] {
            assert!(Solid1x.component_name_may_be_component(component));
        }
        for helper in ["app", "zView"] {
            assert!(!Solid1x.component_name_may_be_component(helper));
        }
    }

    #[test]
    fn function_values_are_not_callbacks_or_owned_computations() {
        let dialect = Solid1x;
        for primitive in [
            Primitive::CreateSignal,
            Primitive::CreateStore,
            Primitive::CreateMutable,
        ] {
            assert!(dialect.callback_positions(primitive).is_empty());
            assert!(dialect.callback_executions(primitive).is_empty());
            assert!(dialect.callback_owners(primitive).is_empty());
            assert_eq!(dialect.callback_execution_at(primitive, 0, 1), None);
            assert_eq!(dialect.callback_owner_at(primitive, 0, 1), None);
            assert_eq!(dialect.cleanup_rule(primitive), CleanupRule::Never);
        }
    }

    #[test]
    fn catch_error_protected_body_has_a_created_owner_but_the_handler_stays_unknown() {
        assert_eq!(
            Solid1x.callback_owner_at(Primitive::CatchError, 0, 2),
            Some(CallbackOwner::Creates)
        );
        assert_eq!(
            Solid1x.callback_owner_at(Primitive::CatchError, 1, 2),
            None,
            "the handler may run immediately under the catch owner or later under the runtime's synthetic error effect"
        );
    }

    #[test]
    fn merge_props_function_sources_are_variadic_tracked_computations() {
        for argument in 0..3 {
            assert_eq!(
                Solid1x.callback_execution_at(Primitive::MergeProps, argument, 3),
                Some(Execution::Tracked)
            );
            assert_eq!(
                Solid1x.callback_owner_at(Primitive::MergeProps, argument, 3),
                Some(CallbackOwner::Creates)
            );
        }
        assert_eq!(
            Solid1x.callback_execution_at(Primitive::MergeProps, 3, 3),
            None
        );
        assert_eq!(Solid1x.callback_owner_at(Primitive::MergeProps, 3, 3), None);
    }

    #[test]
    fn returned_callback_contracts_keep_tuple_slots_and_owner_roles() {
        assert_eq!(
            Solid1x.returned_callback_execution_at(Primitive::UseTransition, Some(0), 0, 1),
            None,
            "the pending accessor is not the transition starter"
        );
        assert_eq!(
            Solid1x.returned_callback_execution_at(Primitive::UseTransition, Some(1), 0, 1),
            Some(Execution::Inline)
        );
        assert_eq!(
            Solid1x.returned_callback_owner_at(Primitive::UseTransition, Some(1), 0, 1),
            Some(CallbackOwner::Inherits)
        );
        assert_eq!(
            Solid1x.returned_callback_owner_at(Primitive::CreateReaction, None, 0, 1),
            Some(CallbackOwner::Creates)
        );
    }

    #[test]
    fn the_table_is_sorted_and_free_of_duplicates() {
        let mut sorted = TABLE.to_vec();
        sorted.sort_by_key(|(name, _)| name.to_lowercase());
        let actual: Vec<_> = TABLE.iter().map(|(name, _)| name.to_lowercase()).collect();
        let expected: Vec<_> = sorted.iter().map(|(name, _)| name.to_lowercase()).collect();
        assert_eq!(actual, expected, "keep the table sorted for review");

        let mut seen = std::collections::HashSet::new();
        for (name, primitive) in TABLE {
            assert!(seen.insert(*name), "{name} is listed twice");
            assert_eq!(reverse(TABLE, *primitive), Some(*name));
        }
    }

    #[test]
    fn suspense_list_is_not_an_async_boundary() {
        // SuspenseList only coordinates the reveal order of child Suspense
        // boundaries -- the 1.9.14 runtime gives it no SuspenseContext, so a
        // pending read beneath one (with no Suspense in between) is NOT
        // bounded by it. The name stays in the vocabulary as a component; it
        // just opens no boundary.
        assert_eq!(Solid1x.boundary_kind("SuspenseList"), None);
        assert_eq!(
            Solid1x.primitive("SuspenseList"),
            Some(Primitive::SuspenseList)
        );
    }

    #[test]
    fn keyed_is_a_boolean_prop_so_the_expression_form_claims_no_accessor() {
        for primitive in [Primitive::Show, Primitive::Match] {
            // Unkeyed (or absent) hands the children callback an accessor.
            assert_eq!(
                Solid1x.children_accessor_parameters(primitive, crate::KeyForm::Absent),
                &[0]
            );
            assert_eq!(
                Solid1x.children_accessor_parameters(primitive, crate::KeyForm::Unkeyed),
                &[0]
            );
            // Keyed hands it the raw value.
            assert!(
                Solid1x
                    .children_accessor_parameters(primitive, crate::KeyForm::Keyed)
                    .is_empty()
            );
            // `keyed={expr}` is a boolean flag in 1.x, not 2.0's key
            // function: truthy picks the raw-value overload, so claiming an
            // accessor here would fabricate a source over a raw value.
            assert!(
                Solid1x
                    .children_accessor_parameters(primitive, crate::KeyForm::CustomKey)
                    .is_empty()
            );
            assert!(
                Solid1x
                    .children_accessor_parameters(primitive, crate::KeyForm::DynamicFlag)
                    .is_empty()
            );
        }
    }

    #[test]
    fn the_store_primitives_are_recognized_but_gated_by_module() {
        // createStore exists in 1.x only under solid-js/store. The name
        // resolves here; whether the import path was legal is the contract
        // layer's question, not the vocabulary's.
        assert_eq!(
            Solid1x.primitive("createStore"),
            Some(Primitive::CreateStore)
        );
        assert!(Solid1x.owns_module("solid-js/store"));
    }

    /// Published 1.9.14 callback-bearing exports intentionally left to another
    /// evidence domain. `createComponent`, `use`, `insert`, and
    /// `getNextElement` are compiler/renderer protocols; the SSR/streaming
    /// helpers vary by package condition; the scheduler/external-source/
    /// observable/renderer hooks have nested or subscription-dependent
    /// contracts the flat dialect table cannot truthfully collapse.
    const UNMODELLED_CALLBACK_TAKERS: &[&str] = &[
        "createComponent",
        "createRenderer",
        "enableExternalSource",
        "enableScheduling",
        "getNextElement",
        "insert",
        "observable",
        "pipeToNodeWritable",
        "pipeToWritable",
        "renderToStream",
        "renderToString",
        "renderToStringAsync",
        "requestCallback",
        "use",
        "useAssets",
    ];

    /// Reviewed against the public declarations in `solid-js@1.9.14`'s
    /// `types/`, `store/types/`, `web/types/`, and `universal/types/` trees.
    /// This includes callback-valued JSX props and returned tuple functions,
    /// not only callbacks already present in the flat package contract.
    const PUBLISHED_CALLBACK_TAKERS: &[&str] = &[
        "ErrorBoundary",
        "For",
        "Index",
        "Match",
        "Show",
        "Switch",
        "batch",
        "catchError",
        "children",
        "createComponent",
        "createComputed",
        "createDynamic",
        "createEffect",
        "createMemo",
        "createReaction",
        "createRenderEffect",
        "createRenderer",
        "createResource",
        "createRoot",
        "createSelector",
        "effect",
        "enableExternalSource",
        "enableScheduling",
        "from",
        "getNextElement",
        "hydrate",
        "indexArray",
        "insert",
        "lazy",
        "mapArray",
        "memo",
        "modifyMutable",
        "observable",
        "on",
        "onCleanup",
        "onError",
        "onMount",
        "pipeToNodeWritable",
        "pipeToWritable",
        "produce",
        "render",
        "renderToStream",
        "renderToString",
        "renderToStringAsync",
        "requestCallback",
        "runWithOwner",
        "startTransition",
        "untrack",
        "use",
        "useAssets",
        "useTransition",
    ];

    #[test]
    fn every_callback_taking_export_is_modelled_or_excluded() {
        let contract = include_str!("../contracts/solid-v1/solid-js.json");
        let exports = serde_json::from_str::<serde_json::Value>(contract).unwrap()["exports"]
            .as_object()
            .unwrap()
            .clone();
        let mut unmodelled = exports
            .iter()
            .filter(|(_, entry)| entry.get("callbacks").is_some())
            .filter_map(|(name, _)| {
                (Solid1x.primitive(name).is_none()
                    && !UNMODELLED_CALLBACK_TAKERS.contains(&name.as_str()))
                .then_some(name.clone())
            })
            .collect::<Vec<_>>();
        unmodelled.sort_unstable();
        assert!(
            unmodelled.is_empty(),
            "Solid 1.x exports declaring callbacks that the vocabulary does not model: {unmodelled:?}"
        );

        for name in UNMODELLED_CALLBACK_TAKERS {
            assert!(
                exports.contains_key(*name),
                "{name} is excluded but is not an export of solid-js any more"
            );
            assert!(
                Solid1x.primitive(name).is_none(),
                "{name} is both excluded and modelled"
            );
        }
    }

    #[test]
    fn every_published_callback_taker_is_modelled_or_has_a_reviewed_exclusion() {
        for name in PUBLISHED_CALLBACK_TAKERS {
            assert!(
                !Solid1x
                    .export_modules(name, crate::ExportPosition::Value)
                    .is_empty(),
                "{name} is in the reviewed callback census but is not a published value export"
            );
            assert!(
                Solid1x.primitive(name).is_some() || UNMODELLED_CALLBACK_TAKERS.contains(name),
                "{name} is callback-bearing but neither modelled nor explicitly excluded"
            );
        }
        for name in UNMODELLED_CALLBACK_TAKERS {
            assert!(
                PUBLISHED_CALLBACK_TAKERS.contains(name),
                "{name} is excluded without appearing in the reviewed callback census"
            );
            assert!(
                Solid1x.primitive(name).is_none(),
                "{name} is both explicitly excluded and modelled"
            );
        }
    }

    #[test]
    fn every_modelled_export_resolves_through_its_namespace_module() {
        for module in Solid1x.modules() {
            let mut expected = TABLE
                .iter()
                .chain(ALIASES)
                .filter_map(|(name, _)| {
                    Solid1x
                        .export_modules(name, crate::ExportPosition::Value)
                        .contains(module)
                        .then_some(*name)
                })
                .collect::<Vec<_>>();
            expected.sort_unstable();
            let mut actual = Solid1x.namespace_import_primitives(module).to_vec();
            actual.sort_unstable();
            assert_eq!(
                actual, expected,
                "namespace imports from {module} must retain every modelled runtime obligation"
            );
        }
    }
}
