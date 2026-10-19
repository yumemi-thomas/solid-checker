//! The primitive vocabulary shared by every dialect, and the small role enums it is described with.

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
    /// `dynamic(source, { static: true })`: the *call form* of
    /// [`Primitive::Dynamic`] whose options object is an exact literal setting
    /// `static` to `true`. Never a name — no table maps a spelling to it; a
    /// call reaches it only through [`Dialect::call_form`], which is where the
    /// dialect states which literal selects it.
    ///
    /// `@solidjs/web@2.0.0-rc.9` answers the option before anything else:
    /// `if (options?.static) return staticDynamic(untrack(source))`
    /// (`dist/web.dev.js:2199`, `dist/web.js:2034`; the server build calls
    /// `untrack(source)` the same way at `dist/server.js:3729-3730`). The
    /// source runs once, untracked, before `dynamic` returns, and no memo is
    /// built — the opposite of the default form's lazy tracked memo.
    DynamicStatic,
    /// `dynamic(source, options)` where `options` does not prove the value of
    /// `static`: an identifier, a spread, a non-literal `static` value, a
    /// getter. The runtime takes one of two forms that disagree on execution,
    /// tracking and ownership, so the dialect models neither; reached only
    /// through [`Dialect::call_form`].
    DynamicUnknownForm,
    /// `until(fn, options?)`, added to `@solidjs/signals` (and re-exported by
    /// `solid-js`) in `2.0.0-rc.9`. The same observer-guarded
    /// `new Promise` + `createRoot` + user-`effect` shape as
    /// [`Primitive::Resolve`], resolving on the first truthy value.
    Until,

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

/// What one call's arguments prove about one boolean option key, as the
/// engine read it for [`Dialect::call_form`].
///
/// The engine reads syntax and nothing else here; which key matters, at which
/// argument, and what each answer selects is the dialect's.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionLiteral {
    /// The key is proven absent at runtime: the argument is missing, a
    /// `null`/`undefined` literal, or an exact object literal (no spread,
    /// computed key or accessor) that does not name the key.
    Absent,
    /// The exact object literal's final `key` property is the literal `true`.
    True,
    /// The exact object literal's final `key` property is the literal `false`.
    False,
    /// Anything else: an identifier, a spread (in the options or in the call's
    /// argument list), a non-literal value, a wrapped literal. The value is
    /// not proven, and a dialect must not pick a form for it.
    Unknown,
}

/// Which part of what a primitive call returns a question is about.
///
/// `Whole` is the returned value itself; `TupleItem(n)` is slot `n` of a
/// returned tuple. The two are not interchangeable and neither is a default:
/// `createMemo()` is an accessor whole, `createSignal()[0]` is an accessor at a
/// slot, and `createSignal()` itself is neither.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResultSlot {
    Whole,
    TupleItem(usize),
}

/// The reactive role of a value: the two roles the normalized contract's shape
/// model represents, and no others.
///
/// Deliberately narrower than [`TypeRole`], which answers what a public *type*
/// export means. This answers what a *call result slot* is, which is the
/// question a provenance proof asks.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ReactiveRole {
    Accessor,
    Setter,
}

/// ADR 0153: the part an export plays in a dialect's context mechanism. See
/// [`Dialect::context_role`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextRole {
    /// `createContext(defaultValue?)`: a fresh context whose provider stores
    /// its `value` prop, and whose default is the first argument.
    Create,
    /// `useContext(context)`: the value stored for `context` on the nearest
    /// owner, else its default; it throws when that is `undefined`.
    Read,
    /// `createComponent(Comp, props)`: runs `Comp(props)` once, on the
    /// caller's stack.
    Render,
}
