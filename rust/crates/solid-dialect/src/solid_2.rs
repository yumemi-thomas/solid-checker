//! Solid 2.0.
//!
//! Every table here started as an extraction of what `solid-reactive-ir`
//! hardcoded before ADR 0006, not a fresh reading of the 2.0 docs; the
//! engine has since been wired onto this crate, so these tables are now the
//! only place the answers live. Provenance stays recorded per table so a
//! change is checked against the runtime it describes, not against memory.
//!
//! Entries added after the original extraction come from the exact published
//! packages (see [`TABLE`]); each needs the same treatment -- a cited source
//! and a fixture or focused regression test.

use crate::{
    Boundary, CallbackOwner, CleanupRule, Dialect, Execution, Primitive, TrackedCallbackTiming,
    Version, lookup, reverse,
};

/// Solid 2.0.
#[derive(Clone, Copy, Debug, Default)]
pub struct Solid2;

/// Source: the pre-ADR-0006 hardcoded name list `solid-reactive-ir` used to
/// carry (26 names), plus the four the namespace-import expansion adds in
/// `solid-reactive-ir/src/symbols.rs` — `merge`, `refresh`, `affects` from
/// `solid-js`, and `dynamic` from `@solidjs/*`. The engine now asks this
/// table; the old list no longer exists.
///
/// `runWithOwner` is the one name here that came from neither. It is extracted
/// from `solid-js@2.0.0-rc.0`, which re-exports it from `@solidjs/signals`
/// as `runWithOwner<T>(owner: Owner | null, fn: () => T): T` — the same
/// signature 1.x has. The engine had always recognised it by spelling whatever
/// the dialect, so before this it resolved to `PrimitiveName::Other` under 2.0
/// and the vocabulary claimed a name 2.0 exports did not exist.
const TABLE: &[(&str, Primitive)] = &[
    ("action", Primitive::Action),
    ("affects", Primitive::Affects),
    ("children", Primitive::Children),
    ("clientOnly", Primitive::ClientOnly),
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
    ("httpHeader", Primitive::HttpHeader),
    ("httpStatus", Primitive::HttpStatus),
    ("hydrate", Primitive::Hydrate),
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
    ("render", Primitive::Render),
    ("Repeat", Primitive::Repeat),
    ("repeat", Primitive::RepeatMap),
    ("resolve", Primitive::Resolve),
    ("runWithOwner", Primitive::RunWithOwner),
    ("Show", Primitive::Show),
    ("snapshot", Primitive::Snapshot),
    ("Switch", Primitive::Switch),
    ("untrack", Primitive::Untrack),
    ("useContext", Primitive::UseContext),
    ("useHead", Primitive::UseHead),
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

    fn direct_jsx_return_is_component(&self) -> bool {
        true
    }

    /// 2.0 folds the store APIs into core and moves the DOM package out.
    /// The web subpaths are owned too: `export_modules` answers with them
    /// for the generated export tables' rows, and a module this dialect
    /// reports exports from is a module it must own.
    fn modules(&self) -> &'static [&'static str] {
        &[
            "solid-js",
            "solid-js/refresh",
            "@solidjs/web",
            "@solidjs/web/frames",
            "@solidjs/web/frames/client",
            "@solidjs/web/frames/server",
            "@solidjs/web/jsx-dev-runtime",
            "@solidjs/web/jsx-runtime",
            "@solidjs/web/serialization",
            "@solidjs/web/serialization/decode",
            "@solidjs/web/server-functions",
            "@solidjs/web/server-functions/client",
            "@solidjs/web/server-functions/rich-args",
            "@solidjs/web/server-functions/server",
            "@solidjs/web/storage",
        ]
    }

    /// `pkg/contracts/bundled/solid-v2/solid-js.json`, the artifact
    /// `solid-facts-backend` compiles in for this dialect.
    fn bundled_contract_label(&self) -> &'static str {
        "solid-v2/solid-js.json"
    }

    fn primitive(&self, name: &str) -> Option<Primitive> {
        static INDEX: crate::NameIndex = crate::NameIndex::new();
        lookup(&INDEX, &[TABLE], name)
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
            | Primitive::ClientOnly
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
            | Primitive::UseHead
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
    ///
    /// `latest(fn)` and `isPending(fn)` are deliberately absent. Each catches
    /// `NotReadyError` around `fn` — that is what they change — but neither
    /// clears tracking, so reads inside them subscribe in the caller's scope
    /// exactly as a bare `fn()` would. Listing them here would erase those
    /// read obligations; their `callback_executions` rows say the same thing.
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
                | Primitive::ClientOnly
                | Primitive::RunWithOwner
                // resolve(fn) returns a Promise -- the thunk's reads settle
                // outside the current computation.
                | Primitive::Resolve
                | Primitive::Lazy
        )
    }

    /// Source: the rc.0 write guard, which exempts children-forbidden
    /// scopes — `!(context._config & CONFIG_CHILDREN_FORBIDDEN)` at the
    /// setter (`dev.js:3154-3172`), `refresh` (`:3316-3331`), and action
    /// (`:4312-4400`) throw sites, with the runtime comment "leaf imperative
    /// scopes (tracked effects, onSettled) stay legal". Empirically probed:
    /// writes, `refresh`, and action calls inside `createTrackedEffect` and
    /// owner-backed `onSettled` succeed on the published rc.0 bundle.
    fn leaf_scopes_allow_writes(&self) -> bool {
        true
    }

    /// Source: rc.0 `untrack` (`dev.js:2928-2942`) clears `tracking` but not
    /// `context`, and the write guard keys on `context`. Probed: a setter,
    /// `refresh`, or action call inside `untrack(...)` within a memo,
    /// component body, or effect compute throws
    /// `REACTIVE_WRITE_IN_OWNED_SCOPE` / `ACTION_CALLED_IN_OWNED_SCOPE`,
    /// while the identical `untrack` call in an event handler succeeds. So
    /// for write legality `untrack` is transparent to its call site. (The
    /// official RFC text claims `untrack` blocks allow writes; the rc.0
    /// runtime contradicts it — the runtime wins.)
    fn callback_preserves_owner_write_context(&self, primitive: Primitive) -> bool {
        primitive == Primitive::Untrack
    }

    /// Source: rc.0 `onSettled` (`dev.js:4855-4893`). Called under a live
    /// children-capable owner it becomes `createTrackedEffect(() =>
    /// untrack(cb))` — a leaf owner where the leaf-scope rules apply. Called
    /// out-of-band (event handler, no owner, inside another leaf) the
    /// callback is enqueued as a plain function: `onCleanup` inside it warns
    /// `NO_OWNER_CLEANUP` instead of throwing, primitives do not throw, and
    /// `flush()` is a silent no-op (all probed). `createTrackedEffect` is a
    /// leaf owner unconditionally and stays out of this list.
    fn leaf_owner_requires_owned_call_site(&self, primitive: Primitive) -> bool {
        primitive == Primitive::OnSettled
    }

    /// `createStore` returns `Readonly<T>` over the root record
    /// (`@solidjs/signals@2.0.0-rc.0`), so a write to one of its own properties
    /// is TS2540 and belongs to TypeScript. Nested records and props objects are
    /// not readonly and stay this checker's.
    fn store_root_properties_are_readonly(&self) -> bool {
        true
    }

    /// Source: rc.0 store setters put the store into the Writing set for the
    /// duration of the draft callback, so `setStore(d => { store.value = 7 })`
    /// commits through the original proxy (probed: the write lands after
    /// `flush()`; the same write through *another* store's proxy inside that
    /// callback is silently dropped, and outside any setter it is silently
    /// dropped too).
    fn store_setter_callback_enables_proxy_writes(&self) -> bool {
        true
    }

    /// Source: rc.0 `devComponent` (`solid-js` dev entry `dev.js:35-50`)
    /// wraps the body in `untrack(() => Comp(props), '<Name>')`, so
    /// `STRICT_READ_UNTRACKED` fires only when a prop getter reads reactive
    /// state during that window. Probed: a component receiving
    /// `{ title: "Hello" }` reads `props.title` in its body with no warning;
    /// the same component receiving `{ get title() { return sig() } }`
    /// warns. Which of the two a prop is is decided entirely by the callers.
    fn props_require_caller_proof(&self) -> bool {
        true
    }

    /// The 2.0 `reactive-read-after-await` page claims store-path and props
    /// member reads; dependency collection genuinely ends at the first await
    /// for those exactly as for accessor calls (`@solidjs/signals` tracks via
    /// the ambient listener, which the resumed continuation no longer has).
    fn reports_member_reads_after_await(&self) -> bool {
        true
    }

    /// Source: RFC 10 — Solid 2.0 moves the `"use server"` directive and the
    /// `@solidjs/web/server-functions` runtime into core, and the pinned
    /// `@solidjs/web@2.0.0-rc.0` ships that runtime (`server-functions/dist`,
    /// probed). 1.x server functions belonged to SolidStart, not this
    /// vocabulary.
    fn models_server_functions(&self) -> bool {
        true
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

    /// Source: the `solid-js@2.0.0-rc.0` and `@solidjs/web@2.0.0-rc.0`
    /// implementations, read
    /// rather than inferred. What matters is whether the body creates an owner
    /// before invoking the callback:
    ///
    /// - `createRoot(init)` is `createOwner()` then `runWithOwner`. Creates.
    /// - `resolve(fn)` wraps `fn` in `createRoot` too, which its signature
    ///   does not suggest -- it reads as a plain thunk-taker.
    /// - `flush(fn)`, `untrack(fn)`, `latest(fn)`, `isPending(fn)` call
    ///   `fn()` directly. No owner is created, so the callback is exactly as
    ///   owned as the call site: Inherits, not None. (What each wraps the
    ///   call in differs — `untrack` clears tracking, `latest`/`isPending`
    ///   only catch `NotReadyError` and leave reads subscribing — but that is
    ///   `runs_callback_deferred`'s question, not this one's.)
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
            // The supplied owner is nullable. The call-site classifier
            // sharpens this to Creates or None when its value is proven.
            Primitive::RunWithOwner => &[(1, CallbackOwner::Conditional)],
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
            // `createReaction` is deliberately absent from this arm: 1.x
            // runs its invalidation callback as a leaf owner, but the
            // RC.0 runtime allocates the reaction a computation like
            // `createEffect` does, so 2.0 does not end the ownership chain
            // there. The `dialect-solid-1x` / `dialect-solid-2` fixture
            // pair pins the difference.
            Primitive::CreateTrackedEffect | Primitive::OnSettled => &[(0, CallbackOwner::Leaf)],
            // Not a leaf (1.x's model — the `dialect-solid-2` fixture pins
            // that difference), but not owned either: the RC.0 runtime
            // invokes the invalidation callback with no owner, and
            // `onCleanup` inside it emits `NO_OWNER_CLEANUP`.
            Primitive::CreateReaction => &[(0, CallbackOwner::None)],
            // Client builds either call the loader at declaration time or on
            // first render; server builds never call it.
            Primitive::ClientOnly => &[(0, CallbackOwner::Conditional)],
            // The browser implementation wraps the thunk in `effect`; SSR
            // registers it for evaluation under the renderer's scope.
            Primitive::UseHead => &[(0, CallbackOwner::Creates)],
            // Both build a row owner -- `_owner: createOwner()` in
            // @solidjs/signals -- so a primitive created in a row callback is
            // disposed with the row rather than leaking.
            Primitive::MapArray | Primitive::RepeatMap => &[(1, CallbackOwner::Creates)],
            Primitive::Flush | Primitive::Untrack | Primitive::Latest | Primitive::IsPending => {
                &[(0, CallbackOwner::Inherits)]
            }
            // Both mount entry points wrap the application callback in the
            // root owner they create (`render` is `createRoot` plus insert).
            Primitive::Render | Primitive::Hydrate => &[(0, CallbackOwner::Creates)],
            // `dynamic(() => Comp)` renders the resolved component under the
            // component owner the wrapper creates, so primitives created in
            // the thunk are disposed with the dynamic node.
            Primitive::Dynamic => &[(0, CallbackOwner::Creates)],
            _ => &[],
        }
    }

    /// Source: `solid-js@2.0.0-rc.0`'s `types/client/flow.d.ts`, read from
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
    /// The dynamic-flag form claims nothing anywhere: RFC 03 says to "avoid
    /// dynamic boolean `keyed` values with function children" precisely
    /// because the callback shape is mode-specific — a truthy flag hands the
    /// callback raw values where the falsy overload hands accessors.
    /// Claiming either shape would fabricate a source for the other, so the
    /// table refuses, mirroring the 1.x dialect's stance on its boolean
    /// `keyed={expr}` (`Show`/`Match` there). `CustomKey` only reaches here
    /// when the key expression is proven a function.
    fn children_accessor_parameters(
        &self,
        primitive: Primitive,
        key: crate::KeyForm,
    ) -> &'static [usize] {
        match primitive {
            // `Show`/`Match` take a boolean `keyed` only (rc.0 flow.d.ts has
            // no key-function overload for them); a function value would be
            // truthy at runtime and select the raw-value overload, so the
            // proven-function form also claims no accessor.
            Primitive::Show | Primitive::Match => match key {
                crate::KeyForm::Keyed | crate::KeyForm::CustomKey | crate::KeyForm::DynamicFlag => {
                    &[]
                }
                crate::KeyForm::Unkeyed | crate::KeyForm::Absent => &[0],
            },
            Primitive::For => match key {
                crate::KeyForm::CustomKey => &[0, 1],
                crate::KeyForm::Unkeyed => &[0],
                crate::KeyForm::Absent | crate::KeyForm::Keyed => &[1],
                crate::KeyForm::DynamicFlag => &[],
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

    /// Spelled out per the trait's rule that an options slot alone is not
    /// evidence for a particular option key. Only the signal-family
    /// constructors route `options.sync` into their node's `CONFIG_SYNC`:
    /// the store family (`createStore(fn, …)`, `createProjection`,
    /// `createOptimisticStore`) rebuilds its node options with only
    /// `loadingValue`/`name` (`@solidjs/signals@2.0.0-rc.0` dev bundle,
    /// `createProjectionInternal`), `sync` is absent from their option
    /// types, and probing confirms a `sync: true` async store derive never
    /// emits `SYNC_NODE_RECEIVED_ASYNC` — the option is inert there, so a
    /// rule keyed on it would flag runtime-legal code.
    fn supports_sync_option(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateMemo
                | Primitive::CreateSignal
                | Primitive::CreateOptimistic
                | Primitive::CreateTrackedEffect
                | Primitive::CreateEffect
                | Primitive::CreateRenderEffect
        )
    }

    /// Source: the checked Solid 2 RC.3 normalized authorities, held to the
    /// receipt-issued bundles by
    /// `the_callback_executions_agree_with_the_bundled_contract`.
    ///
    /// The four `createX(fn, …)` derived forms are not in that table and were
    /// read from the runtime bundled by `solid-js@2.0.0-rc.0` instead:
    /// `createSignal`,
    /// `createOptimistic`, `createStore` and `createOptimisticStore` all branch
    /// on `typeof first === "function"` and build a computed from it, so
    /// argument 0 is a tracked compute exactly when a function is passed.
    ///
    /// `latest` and `isPending` are `Inline` and it is worth saying why, since
    /// reads inside them do subscribe: they run immediately in the caller's
    /// scope and never re-run on their own. `Tracked` here would mean "this
    /// primitive re-runs it", which neither does.
    ///
    /// `flush` is `Inline` too, and since the engine's call graph follows
    /// these execution contracts, a callback handed to `flush` is reachable.
    /// `flushSync(fn)` does invoke `fn`, so the row is the truthful one; its
    /// callbacks are deferred scopes, so that reachability changes no read
    /// or owner diagnostic.
    ///
    /// This row now also decides contract bytes, through
    /// [`Dialect::runs_callback_synchronously`], so the evidence is worth
    /// stating exactly: `@solidjs/signals`' `flush(fn)` is
    /// `syncDepth++; try { return fn() } finally { flush(); syncDepth-- }`
    /// (2.0.0-rc dev bundle), so the callback is invoked and its value returned
    /// **during** the call. Moving it to `Deferred` would publish
    /// `execution: "deferred"` for every package export that forwards a
    /// callback through `flush`, promising the callback has not run when the
    /// export returns — which the runtime contradicts.
    fn callback_executions(&self, primitive: Primitive) -> &'static [(usize, Execution)] {
        match primitive {
            Primitive::CreateMemo
            | Primitive::CreateProjection
            | Primitive::CreateTrackedEffect
            | Primitive::CreateSignal
            | Primitive::CreateStore
            | Primitive::CreateOptimistic
            | Primitive::CreateOptimisticStore
            | Primitive::Dynamic
            | Primitive::UseHead => &[(0, Execution::Tracked)],
            Primitive::CreateEffect | Primitive::CreateRenderEffect => {
                &[(0, Execution::Tracked), (1, Execution::Deferred)]
            }
            Primitive::CreateErrorBoundary | Primitive::CreateLoadingBoundary => {
                &[(0, Execution::Tracked), (1, Execution::Tracked)]
            }
            Primitive::MapArray => &[(1, Execution::Tracked)],
            Primitive::RepeatMap => &[(0, Execution::Tracked), (1, Execution::Inline)],
            Primitive::CreateReaction
            | Primitive::OnSettled
            | Primitive::Resolve
            | Primitive::Lazy
            | Primitive::Action
            | Primitive::ClientOnly => &[(0, Execution::Deferred)],
            Primitive::CreateRoot
            | Primitive::CreateRevealOrder
            | Primitive::Flush
            | Primitive::Untrack
            | Primitive::Latest
            | Primitive::IsPending => &[(0, Execution::Inline)],
            Primitive::RunWithOwner => &[(1, Execution::Inline)],
            // `render(() => <App/>, el)` and `hydrate` invoke the code
            // callback once, immediately, under the root they create.
            // Source: `solidjs-web.json`'s callback rows, the same shape 1.x
            // models for its `solid-js/web` pair.
            Primitive::Render | Primitive::Hydrate => &[(0, Execution::Inline)],
            _ => &[],
        }
    }

    /// Read from `@solidjs/signals@2.0.0-rc.0` `dist/dev.js`, the bundle the
    /// oracle install under `rust/target/tsc-oracle/v2` resolves — line numbers
    /// are that file's. Every answer below was also measured against that
    /// bundle under `--conditions browser` with the probe worker's own
    /// observation shape.
    ///
    /// One line decides most of it: `setupComputedNode` ends with
    /// `!options?.lazy && recompute(self, true)` (`:2845`), so any node built by
    /// `computed(fn, options)` (`:2707-2757`) whose options do not set `lazy`
    /// runs its compute *during* the creating call. That covers:
    ///
    /// - `createMemo` (`:4558-4560`, `accessor(computed(compute, options))`) —
    ///   the public `MemoOptions` has no `lazy` member, so this is
    ///   unconditional. 2.0's memo is **not** pull-based on creation;
    /// - `createSignal(fn)` (`:4548-4552`, the derived overload's
    ///   `computed(first, second)`);
    /// - `createOptimistic(fn)` (`:4778-4790` → `optimisticComputed`,
    ///   `:2888-2892`, which is `computed` plus one field);
    /// - `createProjection` (`:5634-5675`, `node = computed(() => { … }, …)` at
    ///   `:5670` with options that carry only `loadingValue`/`name`).
    ///
    /// `createEffect` (`:4561-4581`) and `createRenderEffect` (`:4610-4612`)
    /// both go through `effect()` (`:4107-4121`), which calls
    /// `recompute(node, true)` unconditionally before queueing the *effect*
    /// function. So the tracked **compute** at argument 0 runs during the call
    /// in 2.0 — the opposite of 1.x's `createEffect`, and the headline dialect
    /// difference on this axis. (Argument 1 is [`Execution::Deferred`], which
    /// is not this method's domain.)
    ///
    /// `createTrackedEffect` (`:4642-4644` → `trackedEffect`, `:4253-4309`) is
    /// the one deferring member: it builds its computed with `lazy: true` and
    /// ends with `node._queue.enqueue(EFFECT_USER, run)` (`:4294`), so nothing
    /// runs before the creating call returns.
    ///
    /// Deliberately unestablished: `createStore` and `createOptimisticStore`
    /// (their derived overloads did not accept the probe's call shape, so no
    /// measurement backs a claim), `dynamic`, `useHead`, the two boundary
    /// primitives and `mapArray`/`repeat`. Contract emission answers the
    /// exact callback leaf open for those rather than assuming they follow `computed`.
    fn tracked_callback_timing(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> Option<TrackedCallbackTiming> {
        if self.callback_execution_at(primitive, argument, argument_count)
            != Some(Execution::Tracked)
        {
            return None;
        }
        match primitive {
            Primitive::CreateMemo
            | Primitive::CreateSignal
            | Primitive::CreateOptimistic
            | Primitive::CreateProjection
            | Primitive::CreateEffect
            | Primitive::CreateRenderEffect => Some(TrackedCallbackTiming::DuringCall),
            Primitive::CreateTrackedEffect => Some(TrackedCallbackTiming::AfterCall),
            _ => None,
        }
    }

    /// `render`/`hydrate` run their code callback once and never again, and
    /// `lazy`'s loader runs once from the wrapper component body, so a
    /// reactive read in any of them is a likely dependency bug — the same
    /// three entry points 1.x reports. `runWithOwner` swaps the owner while
    /// `@solidjs/signals` sets `tracking = false` around the call (see
    /// [`Solid2::runs_callback_deferred`]), so reads inside it register
    /// nothing either. `mapArray`/`repeat` are deliberately absent: unlike
    /// 1.x, their map callbacks run tracked (see the bundled contract rows).
    fn reports_untracked_reads_at(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> bool {
        let _ = argument_count;
        // `createReaction`'s invalidation callback runs untracked and
        // one-shot; the RC.0 runtime emits `STRICT_READ_UNTRACKED` for a
        // reactive read inside it, so the checker reports the same.
        (matches!(
            primitive,
            Primitive::Hydrate | Primitive::Lazy | Primitive::Render | Primitive::CreateReaction
        ) && argument == 0)
            || (primitive == Primitive::RunWithOwner && argument == 1)
    }

    /// The callbacks nothing runs until the returned value is used:
    /// `createReaction`'s invalidation callback arms only once the returned
    /// tracker is called, `lazy`'s loader waits for the wrapper component to
    /// render, and `mapArray(list, mapFn)`/`repeat(count, mapFn)` pull both
    /// their source and their map function through the returned accessor.
    fn callback_requires_return_invocation(&self, primitive: Primitive, argument: usize) -> bool {
        (argument == 0 && matches!(primitive, Primitive::CreateReaction | Primitive::Lazy))
            || (argument <= 1 && primitive == Primitive::MapArray)
            || (argument == 1 && primitive == Primitive::RepeatMap)
    }

    /// `const track = createReaction(onInvalidate); track(() => read())` —
    /// the tracker argument is a tracked computation, the same two-stage
    /// shape as 1.x. Source: the 2.0 cheatsheet's "one-shot tracked
    /// callback" entry and the RC.0 runtime.
    fn returned_callback_execution_at(
        &self,
        primitive: Primitive,
        result_slot: Option<usize>,
        argument: usize,
        argument_count: usize,
    ) -> Option<Execution> {
        match (primitive, result_slot, argument, argument_count) {
            (Primitive::CreateReaction, None, 0, 1..) => Some(Execution::Tracked),
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
            _ => None,
        }
    }

    /// async boundary `Loading` and the error boundary `Errored`.
    fn boundary_name(&self, boundary: Boundary) -> &'static str {
        match boundary {
            Boundary::Async => "Loading",
            Boundary::Error => "Errored",
        }
    }

    /// Source: `solid-reactive-ir/src/cleanup.rs`, both arms of the match —
    /// the unconditional list and the four that depend on the first argument
    /// being a function.
    ///
    /// `createReaction` is a correction to that extraction, not part of it:
    /// the RC.0 runtime allocates a computation for the reaction when it
    /// is called, exactly as `createEffect` does, so creating one in a leaf
    /// or cleanup scope leaks it. Its `creates_directive_owner` row already
    /// recorded the disposal obligation this arm was missing.
    fn cleanup_rule(&self, primitive: Primitive) -> CleanupRule {
        match primitive {
            Primitive::OnCleanup
            | Primitive::Flush
            | Primitive::CreateMemo
            | Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateReaction
            | Primitive::CreateTrackedEffect
            | Primitive::CreateProjection
            | Primitive::CreateRoot
            | Primitive::CreateOwner
            | Primitive::MapArray
            | Primitive::RepeatMap
            | Primitive::CreateRevealOrder
            | Primitive::CreateErrorBoundary
            | Primitive::CreateLoadingBoundary
            | Primitive::UseHead
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

    /// Source: the control-flow component lists `solid-reactive-ir`
    /// hardcoded before ADR 0006, extracted unchanged; this table is now the
    /// only place they live.
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

    /// The removal half of the same probe: a literal `false` removes the
    /// attribute on the client and omits it in SSR (RFC 07 — "Boolean
    /// literals add/remove the attribute"). 1.x stringifies instead, so
    /// only this dialect answers true.
    fn false_attribute_value_removes_attribute(&self) -> bool {
        true
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
                | Primitive::UseHead
        )
    }

    /// Source: `solid-js@2.0.0-rc.0`'s `types/index.d.ts`, which re-exports
    /// the whole vocabulary from the package root. 2.0 folded the store API
    /// into core, so there is no `solid-js/store`; the one primitive that
    /// lives elsewhere is `dynamic`, from the web package.
    /// Two packages, unlike 1.x: 2.0 split the DOM out into `@solidjs/web`,
    /// which has subpaths of its own. A name in both — `render` is not, but
    /// the shape allows it — reports both, and importing it from either
    /// resolves.
    fn export_modules(&self, name: &str, position: crate::ExportPosition) -> Vec<&'static str> {
        let mut found = crate::exports::modules(
            crate::exports::solid_v2_solid_js::VALUES,
            crate::exports::solid_v2_solid_js::TYPES,
            name,
            position,
        );
        for module in crate::exports::modules(
            crate::exports::solid_v2_solidjs_web::VALUES,
            crate::exports::solid_v2_solidjs_web::TYPES,
            name,
            position,
        ) {
            if !found.contains(&module) {
                found.push(module);
            }
        }
        found
    }

    /// Source: `solid-reactive-ir/src/symbols.rs`, `add_solid_import_names`,
    /// which before ADR 0006 matched `"solid-js"` and `"@solidjs/web"` each
    /// exactly. `dynamic` is an `@solidjs/web` root export; a third-party
    /// `@solidjs/*` package exporting a same-spelled `dynamic` is not this
    /// primitive and must not resolve as it.
    fn namespace_import_primitives(&self, module: &str) -> &'static [&'static str] {
        if module == "solid-js" {
            NAMESPACE_SOLID_JS
        } else if module == "@solidjs/web" {
            NAMESPACE_SOLIDJS_WEB
        } else {
            &[]
        }
    }
}

/// The names a `solid-js` namespace import exposes.
///
/// The invariant is the one `solid_1x.rs` enforces: every modelled primitive
/// the module exports must keep its namespace spelling, so a primitive
/// cannot be reachable as `import { x }` but invisible as `Solid.x`. The
/// list used to stop short of that (19 names, `children`/`For`/`Repeat`
/// among them, resolved as nothing through a namespace import — the
/// `namespace-import-v2` fixture pins the difference); the
/// `every_modelled_export_resolves_through_its_namespace_module` test below
/// now derives the expectation from [`TABLE`] and the export census, exactly
/// as the 1.x dialect does.
const NAMESPACE_SOLID_JS: &[&str] = &[
    "Errored",
    "For",
    "Loading",
    "Match",
    "Repeat",
    "Show",
    "Switch",
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
    "flush",
    "getOwner",
    "isPending",
    "latest",
    "lazy",
    "mapArray",
    "merge",
    "omit",
    "onCleanup",
    "onSettled",
    "reconcile",
    "refresh",
    "repeat",
    "resolve",
    "runWithOwner",
    "snapshot",
    "untrack",
    "useContext",
];

/// The names an `@solidjs/web` namespace import exposes, under the same
/// census-derived invariant as [`NAMESPACE_SOLID_JS`]: the entry points plus
/// the control-flow and owner helpers the package re-exports.
const NAMESPACE_SOLIDJS_WEB: &[&str] = &[
    "Errored",
    "For",
    "Loading",
    "Match",
    "Repeat",
    "Show",
    "Switch",
    "clientOnly",
    "dynamic",
    "getOwner",
    "httpHeader",
    "httpStatus",
    "hydrate",
    "render",
    "untrack",
    "useHead",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The three `<For>` overloads from rc.0's `flow.d.ts`, plus the two
    /// forms that must claim nothing: a dynamic boolean flag (either overload
    /// may run — RFC 03 tells authors to prefer a literal or key function),
    /// and — for `Show`/`Match`, whose `keyed` is boolean-only — any
    /// expression form at all.
    #[test]
    fn keyed_forms_claim_only_proven_callback_shapes() {
        use crate::KeyForm;
        assert_eq!(
            Solid2.children_accessor_parameters(Primitive::For, KeyForm::Absent),
            &[1]
        );
        assert_eq!(
            Solid2.children_accessor_parameters(Primitive::For, KeyForm::Keyed),
            &[1]
        );
        assert_eq!(
            Solid2.children_accessor_parameters(Primitive::For, KeyForm::Unkeyed),
            &[0]
        );
        assert_eq!(
            Solid2.children_accessor_parameters(Primitive::For, KeyForm::CustomKey),
            &[0, 1]
        );
        assert!(
            Solid2
                .children_accessor_parameters(Primitive::For, KeyForm::DynamicFlag)
                .is_empty()
        );
        for primitive in [Primitive::Show, Primitive::Match] {
            assert_eq!(
                Solid2.children_accessor_parameters(primitive, KeyForm::Absent),
                &[0]
            );
            for form in [KeyForm::Keyed, KeyForm::CustomKey, KeyForm::DynamicFlag] {
                assert!(
                    Solid2
                        .children_accessor_parameters(primitive, form)
                        .is_empty()
                );
            }
        }
    }

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
                "clientOnly",
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
                "httpHeader",
                "httpStatus",
                "hydrate",
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
                "render",
                "Repeat",
                "repeat",
                "resolve",
                "runWithOwner",
                "Show",
                "snapshot",
                "Switch",
                "untrack",
                "useContext",
                "useHead",
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

    /// The same invariant `solid_1x.rs` enforces: a namespace import must
    /// retain every modelled runtime obligation the module exports, so a
    /// primitive cannot be reachable as `import { x }` but invisible as
    /// `Solid.x`.
    #[test]
    fn every_modelled_export_resolves_through_its_namespace_module() {
        for module in Solid2.modules() {
            let mut expected = TABLE
                .iter()
                .filter_map(|(name, _)| {
                    Solid2
                        .export_modules(name, crate::ExportPosition::Value)
                        .contains(module)
                        .then_some(*name)
                })
                .collect::<Vec<_>>();
            expected.sort_unstable();
            let mut actual = Solid2.namespace_import_primitives(module).to_vec();
            actual.sort_unstable();
            assert_eq!(
                actual, expected,
                "namespace imports from {module} must retain every modelled runtime obligation"
            );
        }
    }

    #[test]
    fn named_and_namespace_imports_can_resolve_run_with_owner() {
        assert!(
            Solid2
                .namespace_import_primitives("solid-js")
                .contains(&"runWithOwner")
        );
    }

    #[test]
    fn rc_0_web_primitives_have_distinct_callback_roles() {
        assert_eq!(Solid2.primitive("clientOnly"), Some(Primitive::ClientOnly));
        assert_eq!(Solid2.primitive("useHead"), Some(Primitive::UseHead));
        assert_eq!(
            Solid2.callback_executions(Primitive::ClientOnly),
            &[(0, Execution::Deferred)]
        );
        assert_eq!(
            Solid2.callback_executions(Primitive::UseHead),
            &[(0, Execution::Tracked)]
        );
        assert!(Solid2.runs_callback_deferred(Primitive::ClientOnly));
        assert!(!Solid2.runs_callback_deferred(Primitive::UseHead));
        assert_eq!(
            Solid2.export_modules("clientOnly", crate::ExportPosition::Value),
            vec!["@solidjs/web"]
        );
        assert_eq!(
            Solid2.export_modules("useHead", crate::ExportPosition::Value),
            vec!["@solidjs/web"]
        );
        assert!(
            Solid2
                .namespace_import_primitives("@solidjs/web")
                .contains(&"clientOnly")
        );
        assert!(
            Solid2
                .namespace_import_primitives("@solidjs/web")
                .contains(&"useHead")
        );
    }

    /// Solid 2.0 exports that take a callback and are deliberately **not**
    /// modelled, with the reason. The list exists so that "every
    /// callback-taking export is in the vocabulary" can be asserted rather
    /// than asserted-with-exceptions-nobody-wrote-down.
    ///
    /// These names are contract-owned helpers rather than dialect primitives.
    /// They stay outside the native vocabulary so their exact
    /// package, artifact-case, and locally open normalized semantics are
    /// consumed through accepted contracts instead of being flattened into
    /// one dialect-wide execution role.
    ///
    /// The generator records the same reviewed callback shapes in the checked-
    /// in contract, which the completeness test below reads directly.
    const UNMODELLED_CALLBACK_TAKERS: &[&str] = &["applyRef", "renderToStream", "renderToString"];

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
        // Both package authorities: `render`/`hydrate` live in `@solidjs/web`,
        // and reading only the core bundle is exactly how an unmodelled mount
        // entry point went unnoticed.
        let exports =
            crate::callback_exports_from_bundles("solid-v2", &["solid-js", "@solidjs/web"]);

        // The contract records callbacks it knows about; the vocabulary is
        // this crate's. Anything in the first and not the second is a gap.
        let mut unmodelled = Vec::new();
        for (name, callbacks) in &exports {
            if callbacks.is_empty() {
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
