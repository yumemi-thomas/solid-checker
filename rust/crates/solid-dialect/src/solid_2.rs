//! Solid 2.0.
//!
//! Every table here started as an extraction of what `solid-reactive-ir`
//! hardcoded before ADR 0006, not a fresh reading of the 2.0 docs. Provenance
//! is recorded per table so that wiring the engine onto this crate can be held
//! to a no-behaviour-change bar.
//!
//! One entry is no longer an extraction. `runWithOwner` was added from the
//! published package (see [`TABLE`]); anything else added the same way needs
//! the same treatment -- a cited source and a fixture.

use crate::{
    Boundary, CallbackOwner, CleanupRule, Dialect, EffectSignature, Execution, Primitive,
    PropsHelpers, Version, lookup, reverse,
};

/// Solid 2.0.
#[derive(Clone, Copy, Debug, Default)]
pub struct Solid2;

/// Source: `solid-reactive-ir/src/lib.rs` `PrimitiveName::as_str` (26 names),
/// plus the four the namespace-import expansion adds in
/// `solid-reactive-ir/src/symbols.rs` — `merge`, `refresh`, `affects` from
/// `solid-js`, and `dynamic` from `@solidjs/*`.
///
/// `runWithOwner` is the one name here that came from neither. It is extracted
/// from `solid-js@2.0.0-beta.19`, which re-exports it from `@solidjs/signals`
/// as `runWithOwner<T>(owner: Owner | null, fn: () => T): T` — the same
/// signature 1.x has. The engine had always recognised it by spelling whatever
/// the dialect, so before this it resolved to `PrimitiveName::Other` under 2.0
/// and the vocabulary claimed a name 2.0 exports did not exist.
const TABLE: &[(&str, Primitive)] = &[
    ("action", Primitive::Action),
    ("affects", Primitive::Affects),
    ("children", Primitive::Children),
    ("createContext", Primitive::CreateContext),
    ("createEffect", Primitive::CreateEffect),
    ("createErrorBoundary", Primitive::CreateErrorBoundary),
    ("createLoadingBoundary", Primitive::CreateLoadingBoundary),
    ("createMemo", Primitive::CreateMemo),
    ("createOptimistic", Primitive::CreateOptimistic),
    ("createOptimisticStore", Primitive::CreateOptimisticStore),
    ("createOwner", Primitive::CreateOwner),
    ("createProjection", Primitive::CreateProjection),
    ("createReaction", Primitive::CreateReaction),
    ("createRenderEffect", Primitive::CreateRenderEffect),
    ("createRevealOrder", Primitive::CreateRevealOrder),
    ("createRoot", Primitive::CreateRoot),
    ("createSignal", Primitive::CreateSignal),
    ("createStore", Primitive::CreateStore),
    ("createTrackedEffect", Primitive::CreateTrackedEffect),
    ("deep", Primitive::Deep),
    ("dynamic", Primitive::Dynamic),
    ("Errored", Primitive::Errored),
    ("flush", Primitive::Flush),
    ("For", Primitive::For),
    ("getOwner", Primitive::GetOwner),
    ("isPending", Primitive::IsPending),
    ("latest", Primitive::Latest),
    ("lazy", Primitive::Lazy),
    ("Loading", Primitive::Loading),
    ("mapArray", Primitive::MapArray),
    ("Match", Primitive::Match),
    ("merge", Primitive::Merge),
    ("omit", Primitive::Omit),
    ("onCleanup", Primitive::OnCleanup),
    ("onSettled", Primitive::OnSettled),
    ("reconcile", Primitive::Reconcile),
    ("refresh", Primitive::Refresh),
    ("Repeat", Primitive::Repeat),
    ("repeat", Primitive::RepeatMap),
    ("resolve", Primitive::Resolve),
    ("runWithOwner", Primitive::RunWithOwner),
    ("Show", Primitive::Show),
    ("snapshot", Primitive::Snapshot),
    ("Switch", Primitive::Switch),
    ("untrack", Primitive::Untrack),
    ("useContext", Primitive::UseContext),
];

/// Every name this dialect exports, derived from [`TABLE`] rather than
/// mirrored beside it. The mirror was a second list to keep in step, and
/// keeping two lists in step by hand is the defect this crate exists to
/// remove one level down.
#[cfg(test)]
pub(crate) fn names() -> Vec<&'static str> {
    TABLE.iter().map(|(name, _)| *name).collect()
}

impl Dialect for Solid2 {
    fn version(&self) -> Version {
        Version::V2
    }

    /// 2.0 folds the store APIs into core and moves the DOM package out.
    fn modules(&self) -> &'static [&'static str] {
        &["solid-js", "@solidjs/web"]
    }

    fn primitive(&self, name: &str) -> Option<Primitive> {
        lookup(TABLE, name)
    }

    fn name_of(&self, primitive: Primitive) -> Option<&'static str> {
        reverse(TABLE, primitive)
    }

    /// Source: `solid-reactive-ir/src/execution_role.rs`, the argument-index
    /// match. `createEffect`/`createRenderEffect` take `(compute, apply)`, so
    /// the tracked callback is at index 1.
    fn callback_positions(&self, primitive: Primitive) -> &'static [usize] {
        match primitive {
            // createEffect/createRenderEffect take (compute, apply);
            // runWithOwner takes (owner, fn).
            Primitive::CreateEffect | Primitive::CreateRenderEffect | Primitive::RunWithOwner => {
                &[1]
            }
            // createErrorBoundary(fn, fallback) and createLoadingBoundary(fn,
            // fallback): argument 0 is the tracked body, argument 1 renders
            // when it throws or suspends.
            Primitive::CreateErrorBoundary | Primitive::CreateLoadingBoundary => &[0, 1],
            // repeat(count, mapFn) and mapArray(list, mapFn) both map at 1.
            Primitive::RepeatMap | Primitive::MapArray => &[1],
            Primitive::CreateMemo
            | Primitive::CreateTrackedEffect
            | Primitive::CreateSignal
            | Primitive::CreateStore
            | Primitive::CreateProjection
            | Primitive::CreateOptimistic
            | Primitive::CreateOptimisticStore
            | Primitive::Dynamic
            | Primitive::Flush
            | Primitive::Untrack
            | Primitive::OnSettled
            | Primitive::CreateReaction
            | Primitive::Action
            // latest(fn), isPending(fn), resolve(fn) each take one thunk.
            | Primitive::Latest
            | Primitive::IsPending
            | Primitive::Resolve
            | Primitive::Lazy
            | Primitive::CreateRevealOrder => &[0],
            _ => &[],
        }
    }

    /// Source: the published signal/runtime implementations, transcribed at
    /// the primitive boundary. Tracked computes such as `createMemo`,
    /// `createTrackedEffect`, derived state/store factories, and `dynamic`
    /// are deliberately absent: putting them here would erase their tracked
    /// read obligations. `createRoot` and `createRevealOrder` are present
    /// because each clears tracking while establishing an owner.
    ///
    /// `runWithOwner` is deferred for the same reason `untrack` is, and on the
    /// same evidence: `@solidjs/signals`' implementation sets `tracking =
    /// false` around the call. It swaps the owner, not the observer, so a read
    /// inside it does not subscribe. 1.x classifies it the same way.
    fn runs_callback_deferred(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateRoot
                | Primitive::CreateRevealOrder
                | Primitive::Flush
                | Primitive::Untrack
                | Primitive::OnSettled
                | Primitive::CreateReaction
                | Primitive::Action
                | Primitive::RunWithOwner
                // resolve(fn) returns a Promise -- the thunk's reads settle
                // outside the current computation. isPending(fn) and
                // latest(fn) deliberately read without subscribing, which is
                // the whole point of them.
                | Primitive::Resolve
                | Primitive::IsPending
                | Primitive::Latest
                | Primitive::Lazy
        )
    }

    /// Source: `solid-reactive-ir/src/lib.rs` `read_is_under_loading` and
    /// `jsx_element_is_loading`. 2.0's error boundary is `Errored`.
    fn boundary_kind(&self, tag: &str) -> Option<Boundary> {
        match tag {
            "Loading" => Some(Boundary::Async),
            "Errored" => Some(Boundary::Error),
            _ => None,
        }
    }

    /// Source: `solid-js@2.0.0-beta.19`'s `types/index.d.ts`. 2.0 spells the
    /// Source: the `@solidjs/signals@2.0.0-beta.25` implementations, read
    /// rather than inferred. What matters is whether the body creates an owner
    /// before invoking the callback:
    ///
    /// - `createRoot(init)` is `createOwner()` then `runWithOwner`. Creates.
    /// - `resolve(fn)` wraps `fn` in `createRoot` too, which its signature
    ///   does not suggest -- it reads as a plain thunk-taker.
    /// - `flush(fn)`, `untrack(fn)`, `latest(fn)`, `isPending(fn)` set a flag
    ///   and call `fn()` directly. No owner is created, so the callback is
    ///   exactly as owned as the call site: Inherits, not None.
    /// - `createEffect(compute, apply)` owns at 0 and runs `apply` unowned,
    ///   which is why these are argument positions and not
    ///   [`Solid2::callback_positions`].
    ///
    /// Anything absent is unmodelled, not ownerless.
    fn callback_owners(&self, primitive: Primitive) -> &'static [(usize, CallbackOwner)] {
        match primitive {
            Primitive::CreateRoot
            | Primitive::CreateMemo
            | Primitive::CreateSignal
            | Primitive::CreateStore
            | Primitive::CreateProjection
            | Primitive::CreateOptimistic
            | Primitive::CreateOptimisticStore
            | Primitive::Resolve => &[(0, CallbackOwner::Creates)],
            // runWithOwner(owner, fn) supplies an owner rather than creating
            // one, and hands it to the function at index 1.
            Primitive::RunWithOwner => &[(1, CallbackOwner::Creates)],
            // createOwner() then runWithOwner(owner, () => fn()).
            Primitive::CreateRevealOrder => &[(0, CallbackOwner::Creates)],
            // The loader is invoked from the wrapper component body.
            Primitive::Lazy => &[(0, CallbackOwner::Inherits)],
            // Both boundaries own the scope their body runs in, and render
            // their fallback under it.
            Primitive::CreateErrorBoundary | Primitive::CreateLoadingBoundary => {
                &[(0, CallbackOwner::Creates), (1, CallbackOwner::Creates)]
            }
            Primitive::CreateEffect | Primitive::CreateRenderEffect => {
                &[(0, CallbackOwner::Creates), (1, CallbackOwner::None)]
            }
            Primitive::CreateTrackedEffect | Primitive::OnSettled => &[(0, CallbackOwner::Leaf)],
            // Both build a row owner -- `_owner: createOwner()` in
            // @solidjs/signals -- so a primitive created in a row callback is
            // disposed with the row rather than leaking.
            Primitive::MapArray | Primitive::RepeatMap => &[(1, CallbackOwner::Creates)],
            Primitive::Flush | Primitive::Untrack | Primitive::Latest | Primitive::IsPending => {
                &[(0, CallbackOwner::Inherits)]
            }
            _ => &[],
        }
    }

    /// Source: `solid-js@2.0.0-beta.19`'s `types/client/flow.d.ts`, read from
    /// the installed package, which spells the three `<For>` forms out:
    ///
    /// ```text
    /// keyed?: true          children: (item: T[number],           index: Accessor<number>)
    /// keyed: false          children: (item: Accessor<T[number]>, index: number)
    /// keyed: (item) => any  children: (item: Accessor<T[number]>, index: Accessor<number>)
    /// ```
    ///
    /// `<Repeat>` is 2.0's answer to `<Index>` and is not the same shape: its
    /// children take `(index: number)`, a plain number, so it has no accessor
    /// parameter at all.
    fn children_accessor_parameters(
        &self,
        primitive: Primitive,
        key: crate::KeyForm,
    ) -> &'static [usize] {
        match primitive {
            Primitive::Show | Primitive::Match => match key {
                crate::KeyForm::Keyed => &[],
                _ => &[0],
            },
            Primitive::For => match key {
                crate::KeyForm::CustomKey => &[0, 1],
                crate::KeyForm::Unkeyed => &[0],
                crate::KeyForm::Absent | crate::KeyForm::Keyed => &[1],
            },
            _ => &[],
        }
    }

    /// `createStore` and `createOptimisticStore` return `[store, setStore]`;
    /// `createProjection` returns the store itself, which the contract also
    /// describes.
    fn returns_store(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateStore | Primitive::CreateOptimisticStore | Primitive::CreateProjection
        )
    }

    /// Source: the match this replaced in `solid-reactive-ir/src/static_api.rs`,
    /// which was 2.0-shaped and correct here. Unchanged on purpose — the point
    /// of moving it was 1.x, where every one of these is a different number.
    ///
    /// `createStore` and `createOptimisticStore` have two forms. The plain
    /// `createStore(value, options?)` puts options at 1 and the derived
    /// `createStore(fn, initial, options?)` at 2; only the derived form takes a
    /// compute, and the rule that asks this is about computes.
    fn options_argument(&self, primitive: Primitive) -> Option<usize> {
        match primitive {
            Primitive::CreateMemo
            | Primitive::CreateSignal
            | Primitive::CreateOptimistic
            | Primitive::CreateTrackedEffect => Some(1),
            Primitive::CreateStore
            | Primitive::CreateProjection
            | Primitive::CreateOptimisticStore
            | Primitive::CreateEffect
            | Primitive::CreateRenderEffect => Some(2),
            _ => None,
        }
    }

    fn supports_sync_option(&self, primitive: Primitive) -> bool {
        self.options_argument(primitive).is_some()
    }

    /// Source: `@solidjs/signals@2.0.0-beta.25`'s implementation, not the
    /// signatures. `createEffect` forwards `effectFn.effect || effectFn` and
    /// `createReaction` calls `(effectFn.effect || effectFn)?.()`;
    /// `createRenderEffect` passes `effectFn` through untouched.
    fn callback_bundle_property(&self, primitive: Primitive) -> Option<&'static str> {
        matches!(
            primitive,
            Primitive::CreateEffect | Primitive::CreateReaction
        )
        .then_some("effect")
    }

    /// Source: the reviewed `SOLID_2` and `SOLIDJS_WEB` semantics tables in
    /// `solid-contract-gen`, which the bundled contract is generated from and
    /// which `the_callback_executions_agree_with_the_bundled_contract` holds
    /// this to.
    ///
    /// The four `createX(fn, …)` derived forms are not in that table and were
    /// read from `@solidjs/signals@2.0.0-beta.25` instead: `createSignal`,
    /// `createOptimistic`, `createStore` and `createOptimisticStore` all branch
    /// on `typeof first === "function"` and build a computed from it, so
    /// argument 0 is a tracked compute exactly when a function is passed.
    ///
    /// `latest` and `isPending` are `Inline` and it is worth saying why, since
    /// reads inside them do subscribe: they run immediately in the caller's
    /// scope and never re-run on their own. `Tracked` here would mean "this
    /// primitive re-runs it", which neither does.
    ///
    /// `flush` is `Inline` too, and since the engine's call graph now follows
    /// these execution contracts, a callback handed to `flush` is reachable —
    /// superseding the old `invokes_its_callbacks` column, which excluded it
    /// while this row said the opposite. `flushSync(fn)` does invoke `fn`, so
    /// the row is the truthful one; its callbacks are deferred scopes, so the
    /// added reachability changes no read or owner diagnostic.
    fn callback_executions(&self, primitive: Primitive) -> &'static [(usize, Execution)] {
        match primitive {
            Primitive::CreateMemo
            | Primitive::CreateProjection
            | Primitive::CreateTrackedEffect
            | Primitive::OnSettled
            | Primitive::CreateSignal
            | Primitive::CreateStore
            | Primitive::CreateOptimistic
            | Primitive::CreateOptimisticStore
            | Primitive::Dynamic => &[(0, Execution::Tracked)],
            Primitive::CreateEffect | Primitive::CreateRenderEffect => {
                &[(0, Execution::Tracked), (1, Execution::Deferred)]
            }
            Primitive::CreateErrorBoundary | Primitive::CreateLoadingBoundary => {
                &[(0, Execution::Tracked), (1, Execution::Deferred)]
            }
            Primitive::MapArray | Primitive::RepeatMap => &[(1, Execution::Tracked)],
            Primitive::CreateReaction
            | Primitive::Resolve
            | Primitive::Lazy
            | Primitive::Action => &[(0, Execution::Deferred)],
            Primitive::CreateRoot
            | Primitive::CreateRevealOrder
            | Primitive::Flush
            | Primitive::Untrack
            | Primitive::Latest
            | Primitive::IsPending => &[(0, Execution::Inline)],
            Primitive::RunWithOwner => &[(1, Execution::Inline)],
            _ => &[],
        }
    }

    /// async boundary `Loading` and the error boundary `Errored`.
    fn boundary_name(&self, boundary: Boundary) -> &'static str {
        match boundary {
            Boundary::Async => "Loading",
            Boundary::Error => "Errored",
        }
    }

    /// Source: `createEffect<T>(compute: ComputeFunction<T>, effect:
    /// EffectFunction<T>, ...)` in `@solidjs/signals`. The split is 2.0's
    /// headline reactivity change and the reason `callback_positions` answers
    /// `[1]` for it.
    fn tracking_scopes(&self) -> &'static str {
        "JSX, a createMemo, or the compute function of createEffect(compute, apply)"
    }

    fn imperative_write_scopes(&self) -> &'static str {
        "an event handler, an action, onSettled, or the apply function of createEffect(compute, apply)"
    }

    fn owned_write_opt_in(&self) -> Option<&'static str> {
        Some("For internal signals only, opt in with createSignal(value, { ownedWrite: true }).")
    }

    fn effect_signature(&self) -> EffectSignature {
        EffectSignature {
            signature: "createEffect(compute, apply)",
            roles: "compute tracks dependencies and returns a value, and apply receives that value and performs the side effect",
            remedy: "Split the callback: reactive reads go in the compute function, the side effect in the apply function, and cleanup is returned from apply. For error handling, pass { effect, error } as the second argument.",
        }
    }

    /// Source: `solid-reactive-ir/src/cleanup.rs`, both arms of the match —
    /// the unconditional list and the four that depend on the first argument
    /// being a function.
    fn cleanup_rule(&self, primitive: Primitive) -> CleanupRule {
        match primitive {
            Primitive::OnCleanup
            | Primitive::Flush
            | Primitive::CreateMemo
            | Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateTrackedEffect
            | Primitive::CreateProjection
            | Primitive::CreateRoot
            | Primitive::CreateOwner
            | Primitive::MapArray
            | Primitive::RepeatMap
            | Primitive::CreateRevealOrder
            | Primitive::CreateErrorBoundary
            | Primitive::CreateLoadingBoundary
            | Primitive::Children => CleanupRule::Always,
            Primitive::CreateSignal
            | Primitive::CreateStore
            | Primitive::CreateOptimistic
            | Primitive::CreateOptimisticStore => CleanupRule::WhenFirstArgumentIsFunction,
            _ => CleanupRule::Never,
        }
    }

    /// Source: the `accepts_cleanup_return` list in
    /// `solid-reactive-ir/src/cleanup.rs`, extracted unchanged.
    fn accepts_cleanup_return(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::OnSettled
                | Primitive::CreateTrackedEffect
                | Primitive::CreateReaction
                | Primitive::CreateEffect
                | Primitive::CreateRenderEffect
        )
    }

    /// Source: the `is_control_flow` and `control_flow_component` lists in
    /// `solid-reactive-ir`, extracted unchanged.
    fn renders_children_through_callback(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::For
                | Primitive::Repeat
                | Primitive::Show
                | Primitive::Match
                | Primitive::Switch
        )
    }

    /// Source: the source-discovery gate in `solid-reactive-ir/src/lib.rs`,
    /// extracted unchanged. Every one returns a tuple or a store the contract's
    /// `returns` column cannot describe.
    fn creates_reactive_source(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateSignal
                | Primitive::CreateMemo
                | Primitive::CreateStore
                | Primitive::CreateProjection
                | Primitive::CreateOptimistic
                | Primitive::CreateOptimisticStore
        )
    }

    /// Source: `solid-reactive-ir/src/directives.rs` `is_created_primitive`.
    fn creates_directive_owner(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateSignal
                | Primitive::CreateMemo
                | Primitive::CreateStore
                | Primitive::CreateProjection
                | Primitive::CreateOptimistic
                | Primitive::CreateOptimisticStore
                | Primitive::CreateEffect
                | Primitive::CreateRenderEffect
                | Primitive::CreateTrackedEffect
                | Primitive::CreateReaction
                | Primitive::CreateRoot
                | Primitive::CreateOwner
                | Primitive::CreateErrorBoundary
                | Primitive::CreateLoadingBoundary
        )
    }

    /// Source: the `component-props-destructure` hint in
    /// `solid-reactive-ir/src/lib.rs`.
    fn props_helpers(&self) -> PropsHelpers {
        PropsHelpers {
            omit: "omit",
            merge: "merge",
        }
    }

    /// Source: `solid-js@2.0.0-beta.19`'s `types/index.d.ts`, which re-exports
    /// the whole vocabulary from the package root. 2.0 folded the store API
    /// into core, so there is no `solid-js/store`; the one primitive that
    /// lives elsewhere is `dynamic`, from the web package.
    /// Two packages, unlike 1.x: 2.0 split the DOM out into `@solidjs/web`,
    /// which has subpaths of its own. A name in both — `render` is not, but
    /// the shape allows it — reports both, and importing it from either
    /// resolves.
    fn export_modules(&self, name: &str, position: crate::ExportPosition) -> Vec<&'static str> {
        let mut found = crate::exports::modules(
            crate::exports::solid_js_2::VALUES,
            crate::exports::solid_js_2::TYPES,
            name,
            position,
        );
        for module in crate::exports::modules(
            crate::exports::solidjs_web::VALUES,
            crate::exports::solidjs_web::TYPES,
            name,
            position,
        ) {
            if !found.contains(&module) {
                found.push(module);
            }
        }
        found
    }

    /// Source: `solid-reactive-ir/src/symbols.rs`, `add_solid_namespace_names`.
    ///
    /// Note the module test: the engine matches `solid-js` exactly but any
    /// `@solidjs/` prefix, so this is not `modules()` membership.
    fn namespace_import_primitives(&self, module: &str) -> &'static [&'static str] {
        if module == "solid-js" {
            NAMESPACE_SOLID_JS
        } else if module.starts_with("@solidjs/") {
            &["dynamic"]
        } else {
            &[]
        }
    }
}

/// The 25 names a `solid-js` namespace import exposes.
///
/// Deliberately narrower than [`TABLE`]: `children`, `For` and `Repeat` are
/// primitives this dialect models, reachable through a direct import or a JSX
/// tag, but the engine has never resolved them through a namespace import or a
/// declaration site. Whether that is a considered exclusion or an oversight is
/// not established, so this preserves it rather than quietly widening the
/// vocabulary. Widening it is a behaviour change and belongs in its own commit
/// with its own fixture.
const NAMESPACE_SOLID_JS: &[&str] = &[
    "createSignal",
    "createMemo",
    "mapArray",
    "createStore",
    "createProjection",
    "createOptimistic",
    "createOptimisticStore",
    "createEffect",
    "createRenderEffect",
    "createTrackedEffect",
    "createReaction",
    "createRoot",
    "createOwner",
    "untrack",
    "onSettled",
    "onCleanup",
    "flush",
    "Loading",
    "Show",
    "Match",
    "Switch",
    "merge",
    "refresh",
    "affects",
    "action",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The vocabulary, pinned by name rather than by count.
    ///
    /// This test used to assert `TABLE.len() == 30` on the grounds that the
    /// table was an extraction of what `solid-reactive-ir` hardcoded, so a
    /// moved count meant drift. That premise expired: the engine now reads
    /// this table rather than the other way round, and the table is sourced
    /// from the published package. A count cannot say which name changed, and
    /// bumping one is not review. A list can and is.
    ///
    /// Adding a name here is a behaviour change -- it starts resolving, and
    /// the fixture snapshots will say what moved.
    #[test]
    fn the_vocabulary_is_pinned_by_name() {
        assert_eq!(
            names(),
            [
                "action",
                "affects",
                "children",
                "createContext",
                "createEffect",
                "createErrorBoundary",
                "createLoadingBoundary",
                "createMemo",
                "createOptimistic",
                "createOptimisticStore",
                "createOwner",
                "createProjection",
                "createReaction",
                "createRenderEffect",
                "createRevealOrder",
                "createRoot",
                "createSignal",
                "createStore",
                "createTrackedEffect",
                "deep",
                "dynamic",
                "Errored",
                "flush",
                "For",
                "getOwner",
                "isPending",
                "latest",
                "lazy",
                "Loading",
                "mapArray",
                "Match",
                "merge",
                "omit",
                "onCleanup",
                "onSettled",
                "reconcile",
                "refresh",
                "Repeat",
                "repeat",
                "resolve",
                "runWithOwner",
                "Show",
                "snapshot",
                "Switch",
                "untrack",
                "useContext",
            ]
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
            assert_eq!(
                reverse(TABLE, *primitive),
                Some(*name),
                "{name} maps to a primitive another name already claims"
            );
        }
    }

    /// Solid 2.0 exports that take a callback and are deliberately **not**
    /// modelled, with the reason. The list exists so that "every
    /// callback-taking export is in the vocabulary" can be asserted rather
    /// than asserted-with-exceptions-nobody-wrote-down.
    ///
    /// All three are internals. `createComponent` and `devComponent` are
    /// emitted by the JSX transform; `ssrScope` runs only on the server entry
    /// point. None appears in application code, and modelling one would mean
    /// inventing behavioural columns nothing could check.
    ///
    /// Mirrored by `EXCLUDED` in `tasks/contracts/solid-js-contract.test.mjs`,
    /// which checks the same list against the installed package.
    const UNMODELLED_CALLBACK_TAKERS: &[&str] = &["createComponent", "devComponent", "ssrScope"];

    /// Every `solid-js` 2.0 export that takes a callback is either in the
    /// vocabulary or on the exclusion list above.
    ///
    /// This is the completeness criterion for the 2.0 side. A name that takes
    /// a callback is one whose body the engine may need to reason about --
    /// who owns it, whether its reads track, whether calling the primitive
    /// reaches it -- and a name absent from the vocabulary answers none of
    /// those questions. Names that take no callback and return no reactive
    /// value carry no obligation and are out of scope by construction.
    ///
    /// Reads the checked-in contract rather than `node_modules`, so it runs
    /// without an install; the contract is verified against the published
    /// package by `contracts_process.rs`.
    #[test]
    fn every_callback_taking_export_is_modelled_or_excluded() {
        let contract = include_str!("../contracts/solid-js.json");
        let exports: std::collections::BTreeMap<String, serde_json::Value> =
            serde_json::from_str::<serde_json::Value>(contract).unwrap()["exports"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

        // The contract records callbacks it knows about; the vocabulary is
        // this crate's. Anything in the first and not the second is a gap.
        let mut unmodelled = Vec::new();
        for (name, entry) in &exports {
            if entry.get("callbacks").is_none() {
                continue;
            }
            if Solid2.primitive(name).is_none()
                && !UNMODELLED_CALLBACK_TAKERS
                    .iter()
                    .any(|excluded| excluded == name)
            {
                unmodelled.push(name.clone());
            }
        }
        assert!(
            unmodelled.is_empty(),
            "solid-js exports declaring callbacks that the vocabulary does not model: {unmodelled:?}"
        );

        // The exclusions must stay real exports; a stale one hides a gap.
        for name in UNMODELLED_CALLBACK_TAKERS {
            assert!(
                exports.contains_key(*name),
                "{name} is excluded but is not an export of solid-js any more"
            );
            assert!(
                Solid2.primitive(name).is_none(),
                "{name} is on the exclusion list and in the vocabulary; pick one"
            );
        }
    }

    #[test]
    fn only_callback_bearing_primitives_report_positions() {
        // A tag is not a call; asking for its callback positions is a caller
        // error that should answer empty rather than guess index 0.
        assert!(Solid2.callback_positions(Primitive::For).is_empty());
        assert!(Solid2.callback_positions(Primitive::Loading).is_empty());
        assert!(Solid2.callback_positions(Primitive::Children).is_empty());
    }
}
