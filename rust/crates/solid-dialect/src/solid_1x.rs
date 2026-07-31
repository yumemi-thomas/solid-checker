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
    Boundary, CallbackOwner, CleanupRule, Dialect, EffectSignature, Execution, Primitive,
    PropsHelpers, Version, lookup, reverse,
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
    ("createDeferred", Primitive::CreateDeferred),
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
    ("getOwner", Primitive::GetOwner),
    ("Index", Primitive::Index),
    ("indexArray", Primitive::IndexArray),
    ("mapArray", Primitive::MapArray),
    ("Match", Primitive::Match),
    ("mergeProps", Primitive::MergeProps),
    ("modifyMutable", Primitive::ModifyMutable),
    ("on", Primitive::On),
    ("onCleanup", Primitive::OnCleanup),
    ("onError", Primitive::OnError),
    ("onMount", Primitive::OnMount),
    ("produce", Primitive::Produce),
    ("reconcile", Primitive::Reconcile),
    ("runWithOwner", Primitive::RunWithOwner),
    ("Show", Primitive::Show),
    ("splitProps", Primitive::SplitProps),
    ("startTransition", Primitive::StartTransition),
    ("Suspense", Primitive::Suspense),
    ("SuspenseList", Primitive::SuspenseList),
    ("Switch", Primitive::Switch),
    ("untrack", Primitive::Untrack),
    ("unwrap", Primitive::Unwrap),
    ("useTransition", Primitive::UseTransition),
];

/// Every name this dialect exports, derived from [`TABLE`] rather than
/// mirrored beside it. The mirror was a second list to keep in step, and
/// keeping two lists in step by hand is the defect this crate exists to
/// remove one level down.
#[cfg(test)]
pub(crate) fn names() -> Vec<&'static str> {
    TABLE.iter().map(|(name, _)| *name).collect()
}

impl Dialect for Solid1x {
    fn version(&self) -> Version {
        Version::V1
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

    fn primitive(&self, name: &str) -> Option<Primitive> {
        lookup(TABLE, name)
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
    /// It is also a mistake already present in the tree. The `1.x` branch
    /// retargeted `callback_argument_index` to 0 but left
    /// `allowed_callback_spans` at 1, so one site there reads the seed value
    /// as the callback. Wiring the engine onto this table fixes that site and
    /// will move findings on `createEffect(fn, seed)`; that is a correction,
    /// not a regression, and it is the kind of divergence one vocabulary
    /// exists to prevent.
    fn callback_positions(&self, primitive: Primitive) -> &'static [usize] {
        match primitive {
            Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateComputed
            | Primitive::CreateMemo
            | Primitive::CreateDeferred
            | Primitive::CreateSelector
            | Primitive::CreateReaction
            | Primitive::CreateRoot
            | Primitive::Untrack
            | Primitive::Batch
            | Primitive::StartTransition
            | Primitive::OnMount
            | Primitive::OnCleanup
            | Primitive::OnError
            | Primitive::CatchError
            | Primitive::Produce => &[0],
            // createResource(source, fetcher) — the fetcher may sit at either
            // index depending on whether a source is supplied.
            Primitive::CreateResource => &[0, 1],
            // mapArray(list, mapFn) / indexArray(list, mapFn)
            Primitive::MapArray | Primitive::IndexArray => &[1],
            // runWithOwner(owner, fn)
            Primitive::RunWithOwner => &[1],
            _ => &[],
        }
    }

    /// 1.x defers a wider set than 2.0: it keeps `batch` and the transition
    /// helpers, and its lifecycle callbacks run outside the creating
    /// computation's tracked scope.
    ///
    /// Unproven, like the rest of the 1.x behavioural columns -- derived from
    /// the semantics in `docs/solid-1x-api-surface.md`, not extracted from a
    /// working 1.x engine.
    fn runs_callback_deferred(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::Untrack
                | Primitive::Batch
                | Primitive::StartTransition
                | Primitive::CreateReaction
                | Primitive::OnMount
                | Primitive::OnCleanup
                | Primitive::OnError
                | Primitive::CatchError
                | Primitive::RunWithOwner
        )
    }

    /// 1.x's async boundary is `Suspense`; its error boundary is
    /// `ErrorBoundary`. There is no `Loading` and no `Errored` —
    /// `docs/solid-1x-api-surface.md` records `Errored` as a false positive,
    /// present only as the `"errored"` member of `createResource`'s state
    /// union.
    fn boundary_kind(&self, tag: &str) -> Option<Boundary> {
        match tag {
            "Suspense" | "SuspenseList" => Some(Boundary::Async),
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
    /// Unproven in the same way the rest of the 1.x behavioural columns are:
    /// derived from the semantics in `docs/solid-1x-api-surface.md`, not
    /// extracted from a running 1.x engine the way the 2.0 side was.
    fn callback_owners(&self, primitive: Primitive) -> &'static [(usize, CallbackOwner)] {
        match primitive {
            Primitive::CreateRoot
            | Primitive::CreateMemo
            | Primitive::CreateSignal
            | Primitive::CreateStore
            | Primitive::CreateMutable
            | Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateComputed
            | Primitive::CreateDeferred
            | Primitive::CreateSelector => &[(0, CallbackOwner::Creates)],
            Primitive::RunWithOwner => &[(1, CallbackOwner::Creates)],
            Primitive::CreateReaction => &[(0, CallbackOwner::Leaf)],
            // createResource(source, fetcher): the fetcher runs outside the
            // creating computation, and its position depends on whether a
            // source was supplied.
            Primitive::CreateResource => &[(0, CallbackOwner::None), (1, CallbackOwner::None)],
            Primitive::MapArray | Primitive::IndexArray => &[(1, CallbackOwner::Creates)],
            Primitive::Untrack | Primitive::Batch | Primitive::StartTransition => {
                &[(0, CallbackOwner::Inherits)]
            }
            _ => &[],
        }
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
            Primitive::Show | Primitive::Match => match key {
                crate::KeyForm::Keyed => &[],
                _ => &[0],
            },
            Primitive::For => &[1],
            Primitive::Index => &[0],
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
            | Primitive::CreateSelector => Some(2),
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
            | Primitive::CreateDeferred => &[(0, Execution::Tracked)],
            Primitive::CreateReaction
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
            | Primitive::StartTransition
            | Primitive::Produce => &[(0, Execution::Inline)],
            Primitive::RunWithOwner | Primitive::ModifyMutable => &[(1, Execution::Inline)],
            _ => &[],
        }
    }

    /// 1.x's own set. Before this was a dialect question the engine used the
    /// 2.0 list for both dialects, which left `batch`, `startTransition`,
    /// `mapArray`, `onMount` and the rest of 1.x's callback-takers out of the
    /// call graph entirely.
    ///
    /// Unproven in the same way the other 1.x behavioural columns are.
    fn invokes_its_callbacks(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::Batch
                | Primitive::CatchError
                | Primitive::CreateComputed
                | Primitive::CreateDeferred
                | Primitive::CreateEffect
                | Primitive::CreateMemo
                | Primitive::CreateMutable
                | Primitive::CreateReaction
                | Primitive::CreateRenderEffect
                | Primitive::CreateResource
                | Primitive::CreateRoot
                | Primitive::CreateSelector
                | Primitive::CreateSignal
                | Primitive::CreateStore
                | Primitive::IndexArray
                | Primitive::MapArray
                | Primitive::OnCleanup
                | Primitive::OnError
                | Primitive::OnMount
                | Primitive::Produce
                | Primitive::RunWithOwner
                | Primitive::StartTransition
                | Primitive::Untrack
        )
    }

    /// `Suspense`, not `Loading`; `ErrorBoundary`, not `Errored`. Source:
    /// `docs/solid-1x-api-surface.md`, the control-flow components.
    ///
    /// `SuspenseList` also opens an async boundary, so this is not a total
    /// inverse of [`Solid1x::boundary_kind`] -- it names the one to *suggest*,
    /// and suggesting `SuspenseList` for a single pending read would be wrong.
    fn boundary_name(&self, boundary: Boundary) -> &'static str {
        match boundary {
            Boundary::Async => "Suspense",
            Boundary::Error => "ErrorBoundary",
        }
    }

    /// `createEffect<Next, Init>(fn, value?, options?)`. Source: the signature
    /// quoted in `docs/solid-1x-api-surface.md`.
    ///
    /// One callback, at index 0. Index 1 is a seed value, not a second
    /// callback -- see [`Solid1x::callback_positions`] for why reading it as
    /// 2.0's apply is the highest-yield mistake available in this file.
    /// 1.x's effect callback is one function, so there is no compute to name.
    fn tracking_scopes(&self) -> &'static str {
        "JSX, a createMemo, or the callback of createEffect(fn)"
    }

    /// Neither `action` nor `onSettled` exists in 1.x, and its effect callback
    /// has no apply half. What 1.x does have is `onMount`, which runs once
    /// after the initial render and outside the tracking scope.
    fn imperative_write_scopes(&self) -> &'static str {
        "an event handler, onMount, or a callback that runs after the current computation"
    }

    fn effect_signature(&self) -> EffectSignature {
        EffectSignature {
            signature: "createEffect(fn, value?)",
            roles: "fn tracks dependencies and runs the side effect, and the optional value seeds the previous value passed to fn on its first run",
            remedy: "Pass the effect function as the first argument. Reads inside it are tracked, and cleanup is registered with onCleanup rather than returned.",
        }
    }

    /// The 2.0 rule minus the primitives 1.x does not have, plus 1.x's own
    /// owner-creating primitives. `createSignal` keeps the conditional form:
    /// 1.x has no `ownedWrite` option, so the first-argument-is-a-function
    /// shape is the only signal.
    fn cleanup_rule(&self, primitive: Primitive) -> CleanupRule {
        match primitive {
            Primitive::OnCleanup
            | Primitive::OnMount
            | Primitive::CreateMemo
            | Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateComputed
            | Primitive::CreateDeferred
            | Primitive::CreateSelector
            | Primitive::CreateRoot
            | Primitive::MapArray
            | Primitive::IndexArray
            | Primitive::Children => CleanupRule::Always,
            Primitive::CreateSignal | Primitive::CreateStore | Primitive::CreateMutable => {
                CleanupRule::WhenFirstArgumentIsFunction
            }
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
    /// `createMutable` returns a store, and neither shape fits the bundled
    /// contract's single-value `returns` column — so before this, a read
    /// through either was traced to no source and reported nowhere.
    ///
    /// `createComputed` is absent on purpose: it returns nothing.
    fn creates_reactive_source(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateSignal
                | Primitive::CreateMemo
                | Primitive::CreateStore
                | Primitive::CreateMutable
                | Primitive::CreateResource
                | Primitive::CreateDeferred
                | Primitive::CreateSelector
        )
    }

    fn creates_directive_owner(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateSignal
                | Primitive::CreateMemo
                | Primitive::CreateStore
                | Primitive::CreateMutable
                | Primitive::CreateEffect
                | Primitive::CreateRenderEffect
                | Primitive::CreateComputed
                | Primitive::CreateDeferred
                | Primitive::CreateSelector
                | Primitive::CreateReaction
                | Primitive::CreateRoot
                | Primitive::CreateResource
        )
    }

    fn props_helpers(&self) -> PropsHelpers {
        PropsHelpers {
            omit: "splitProps",
            merge: "mergeProps",
        }
    }

    /// Source: `docs/solid-1x-api-surface.md`, the `solid-js` and
    /// `solid-js/store` sections. The split is the difference that makes
    /// module gating worth having: every name below sits in core in 2.0.
    /// Straight out of the generated index: 1.x ships one package with four
    /// subpaths, so one table answers for all of them.
    fn export_modules(&self, name: &str, position: crate::ExportPosition) -> Vec<&'static str> {
        crate::exports::modules(
            crate::exports::solid_js_1x::VALUES,
            crate::exports::solid_js_1x::TYPES,
            name,
            position,
        )
    }

    /// Per-subpath, which is the whole reason this is module-scoped:
    /// `solid-js/store` exposes the store primitives and core does not.
    ///
    /// Unproven. No 1.x fixture exercises a namespace import yet, so unlike
    /// the 2.0 side this is derived from `docs/solid-1x-api-surface.md` rather
    /// than extracted from a working engine.
    fn namespace_import_primitives(&self, module: &str) -> &'static [&'static str] {
        match module {
            "solid-js" => NAMESPACE_CORE,
            "solid-js/store" => NAMESPACE_STORE,
            _ => &[],
        }
    }
}

const NAMESPACE_CORE: &[&str] = &[
    "createSignal",
    "createMemo",
    "createEffect",
    "createRenderEffect",
    "createComputed",
    "createReaction",
    "createDeferred",
    "createSelector",
    "createResource",
    "createRoot",
    "untrack",
    "batch",
    "on",
    "startTransition",
    "onMount",
    "onCleanup",
    "onError",
    "catchError",
    "getOwner",
    "runWithOwner",
    "mapArray",
    "indexArray",
    "mergeProps",
    "splitProps",
];

const NAMESPACE_STORE: &[&str] = &[
    "createStore",
    "produce",
    "reconcile",
    "unwrap",
    "createMutable",
    "modifyMutable",
];

#[cfg(test)]
mod tests {
    use super::*;

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
    fn suspense_list_is_an_async_boundary_too() {
        // SuspenseList coordinates Suspense boundaries; a pending read beneath
        // one is still bounded.
        assert_eq!(Solid1x.boundary_kind("SuspenseList"), Some(Boundary::Async));
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
}
