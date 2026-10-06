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
mod solid_2;

pub use exports::Position as ExportPosition;
pub use solid_2::Solid2;

/// Which Solid language version a project targets.
///
/// Distinct from every wire and schema version in this repository. Type Facts
/// v2, the execution-facts protocol, and `solid-reactivity.json` all carry
/// version numbers of their own that have nothing to do with the Solid
/// version — see ADR 0006, trap 1.
/// Variants are declared in ascending Solid-version order and `Ord` follows
/// that declaration, so "the newest major this build knows about" is a
/// `max` rather than a literal somebody has to remember to change.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Version {
    /// Solid 1.x — `Suspense`, `createResource`, `createEffect(fn)`.
    V1,
    /// Solid 2.0 — `Loading`, `createTrackedEffect`,
    /// `createEffect(compute, apply)`.
    V2,
}

/// Every vocabulary this build carries, in ascending version order.
///
/// The cross-dialect helpers below ask their question of each entry and answer
/// only where the answers agree; the tests that loop it assert one property
/// per vocabulary. Both iterate *this* rather than enumerating [`Version`]
/// because the question is "what do the vocabularies I have say", which is not
/// the same question as "which Solid versions exist" — [`Version::V1`] is
/// retained for classification long after a build stops carrying its
/// vocabulary, so detection can recognise an installed 1.x runtime in order to
/// refuse it rather than silently analyzing it as 2.0 (ADR 0110).
///
/// Retiring a dialect is therefore an edit *here*, plus deleting whatever is
/// genuinely differential. Nothing that merely happens to run over every
/// vocabulary needs touching.
pub const DIALECTS: &[&'static dyn Dialect] = &[&Solid2];

impl Version {
    /// The adapter for this version, when this build carries one.
    ///
    /// [`Version::V1`] answers `None`, and the variant is deliberately kept
    /// with no vocabulary behind it. *Classifying* an installed runtime and
    /// *analyzing* it are different questions: detection has to recognise
    /// `1.9.14` as a released major in order to refuse it
    /// (`SC9013 unsupported-solid-runtime`).
    ///
    /// A major with no variant at all is not a hole for the same reason —
    /// [`Version::for_solid_js`] answers [`Classification::UnmodelledMajor`]
    /// and detection refuses that identically. The variant buys a *name* for
    /// the refusal, not the refusal itself.
    ///
    /// Use [`DIALECTS`] to ask what the vocabularies on hand say; use this
    /// only when a specific version's vocabulary is the question.
    #[must_use]
    pub fn dialect(self) -> Option<&'static dyn Dialect> {
        match self {
            Self::V1 => None,
            Self::V2 => Some(&Solid2),
        }
    }

    /// What a resolved `solid-js` version string says about the language the
    /// project runs.
    ///
    /// Takes the major component of a semver string and nothing else, so
    /// `2.0.0-rc.0` is 2.0 and `1.9.14` is 1.x. Prerelease and build metadata
    /// are ignored deliberately: refusing to classify a 2.0 prerelease would
    /// leave real RC projects on the fallback. Range prefixes (`^`, `~`,
    /// `>=`, `<`) are stripped for
    /// the same reason, although the detection path only ever passes exact
    /// installed versions.
    ///
    /// `None` means the string is **not a version** — `workspace:*`, an empty
    /// field, anything whose leading component will not parse. That is an
    /// absence: the caller falls back to its own default, which is what every
    /// request without a resolvable `solid-js` has always received.
    ///
    /// A number that *is* a major always classifies, even when no variant here
    /// names it, because [`Classification::UnmodelledMajor`] and `None` need
    /// different answers from the caller and collapsing them is how an
    /// installed `solid-js@3.0.0` would get analyzed as 2.0 in silence — the
    /// hole ADR 0110 § 1 closed for 1.x, reopened for every major after this
    /// one.
    #[must_use]
    pub fn for_solid_js(version: &str) -> Option<Classification> {
        let major = version
            .trim()
            .trim_start_matches(['^', '~', '=', 'v', ' ', '>', '<'])
            .split(['.', '-', '+'])
            .next()?
            .parse::<u32>()
            .ok()?;
        Some(match major {
            1 => Classification::Modelled(Self::V1),
            2 => Classification::Modelled(Self::V2),
            other => Classification::UnmodelledMajor(other),
        })
    }
}

/// What an installed `solid-js` version string says about the language a
/// project runs.
///
/// The distinction this type exists for is between a major this build has a
/// *name* for and one it does not. Both may end in a refusal — [`Version::V1`]
/// is named and carries no vocabulary — but only one of them can say which
/// version it refused, and only one of them is a question the next dialect
/// answers by adding a variant.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Classification {
    /// A major [`Version`] names. Whether this build also carries its
    /// *vocabulary* is the separate question [`Version::dialect`] answers.
    Modelled(Version),
    /// A released major no [`Version`] variant names, carrying that major.
    ///
    /// This is a contradicted answer, not an absence: the project imports a
    /// Solid this build does not speak, and analyzing it under the default
    /// catalog would report findings in a language it does not run. Detection
    /// refuses it with `SC9013` for exactly the reason it refuses 1.x.
    UnmodelledMajor(u32),
}

/// A package whose installed release decides some of a vocabulary's answers,
/// and where a project's code resolves it from.
///
/// [`Version::for_solid_js`] reads the `solid-js` major and nothing else,
/// because a major is what selects a *language*. Within a major the published
/// bytes still move between prereleases, and not only in `solid-js`: Solid 2
/// splits its runtime across three archives, and the answer a rule rests on
/// belongs to whichever of them declares it (the store typing to
/// `@solidjs/signals`, `dynamic` to `@solidjs/web`). Each such package is named
/// here so shared code can resolve it without knowing why it matters
/// ([`Dialect::release_owners`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseOwner {
    /// The package name, exactly as a manifest spells it.
    pub package: &'static str,
    /// `None`: resolved from the project, the way the project's own import of
    /// it resolves. `Some(owner)`: resolved from the installed directory of
    /// another owner, the way *that* package's import of it resolves -- which
    /// is where a re-export's declarations come from.
    pub resolved_from: Option<&'static str>,
}

/// One [`ReleaseOwner`] as resolved for a project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstalledRelease<'a> {
    /// The owner's package name.
    pub package: &'a str,
    /// The resolved manifest's `version` field exactly as spelled, or `None`
    /// when no manifest carrying a version string resolved at all.
    pub version: Option<&'a str>,
}

/// How a vocabulary stands against one project's installation of the packages
/// it names ([`Dialect::review_installation`]).
///
/// The answer is the dialect's, never shared code's, because only the dialect
/// knows which releases it was read on and which package owns which answer.
#[derive(Clone)]
pub enum InstallationReview {
    /// Analysis proceeds.
    Analyzed {
        /// The vocabulary that answers for this installation, or `None` for
        /// the language's own ([`Dialect::variant_key`] `None`).
        vocabulary: Option<&'static dyn Dialect>,
        /// Every open gap in what the dialect knows about the installation:
        /// an owner at a release reviewed with gaps, an owner at a release
        /// nobody compared, an owner that did not resolve, or owners at
        /// releases no review read together. Empty means every owner resolved
        /// to a release the vocabulary was read on, and analysis says nothing
        /// about the installation; anything else makes the result
        /// uncertifiable, with one project-level notice naming each gap.
        gaps: Vec<InstallationGap>,
    },
    /// A release whose runtime the vocabulary does not describe, even though
    /// its major matches. Refused exactly like an uncarried major.
    Refused(&'static RefusedRelease),
}

impl std::fmt::Debug for InstallationReview {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Analyzed { vocabulary, gaps } => formatter
                .debug_struct("Analyzed")
                .field(
                    "vocabulary",
                    &vocabulary.map(|vocabulary| vocabulary.variant_key()),
                )
                .field("gaps", gaps)
                .finish(),
            Self::Refused(refusal) => formatter.debug_tuple("Refused").field(refusal).finish(),
        }
    }
}

/// One open gap in what a vocabulary knows about an installation, as one
/// sentence a user can act on, and the review that measured it, if any.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallationGap {
    pub gap: String,
    /// The review document, repository-relative. `None` for a gap no review
    /// covers: an owner nobody compared, or one that did not resolve.
    pub review: Option<&'static str>,
    /// `None`: the gap is open for every project on the installation.
    /// `Some`: it is open only for a project whose sources reach one of the
    /// named exports ([`GapScope`]), and shared code decides that from the
    /// project's import facts before the notice is due.
    pub scope: Option<GapScope>,
}

/// The exports a scoped [`InstallationGap`] is about: the gap is due only for
/// a project that reaches one of `exports` through the module `specifier`
/// resolves to, or whose use of that module the facts cannot bound to named
/// exports (a namespace object that escapes, `export *`, a dynamic load).
///
/// The dialect names the module and the export names; shared code knows only
/// how an ECMAScript module reaches a named export, never why these names
/// matter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GapScope {
    /// The bare specifier of the module whose exports these are, exactly as
    /// an import spells it (`solid-js`, never a subpath).
    pub specifier: &'static str,
    /// Export names, exactly as the module's declarations spell them.
    pub exports: &'static [&'static str],
}

/// A release line refused within a modelled major, and why.
#[derive(Debug, Eq, PartialEq)]
pub struct RefusedRelease {
    /// The line, as a user would recognise it (`2.0.0-experimental.x`).
    pub line: &'static str,
    /// Why the vocabulary cannot model it: the runtime difference, stated.
    pub reason: &'static str,
    /// Where that difference was measured, repository-relative.
    pub review: &'static str,
    /// The installation the refusing vocabulary was audited on
    /// ([`Dialect::audited_installation`]), which the refusal names as the
    /// one to move to.
    pub audited: &'static [(&'static str, &'static str)],
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

/// Returns an owner-requirement role only when every dialect that recognizes
/// the exact export name assigns the same role. An unrecognized or disputed
/// name stays open rather than inheriting behavior from another dialect.
#[must_use]
pub fn unambiguous_owner_requirement_role(name: &str) -> Option<OwnerRequirementRole> {
    let mut roles = DIALECTS
        .iter()
        .copied()
        .filter_map(|dialect| {
            let primitive = dialect.primitive(name)?;
            (dialect.name_of(primitive) == Some(name)).then_some((dialect, primitive))
        })
        .filter_map(|(dialect, primitive)| dialect.owner_requirement_role(primitive));
    let first = roles.next()?;
    roles.all(|role| role == first).then_some(first)
}

/// Returns true only when every dialect that canonically exports `name`
/// identifies `argument` as a callback for this exact call shape.
#[must_use]
pub fn unambiguous_callback_argument(name: &str, argument: usize, argument_count: usize) -> bool {
    let answers = DIALECTS
        .iter()
        .copied()
        .filter_map(|dialect| {
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
    let roles = DIALECTS
        .iter()
        .filter_map(|dialect| dialect.type_role(origin_module, name))
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
    let answers = DIALECTS
        .iter()
        .copied()
        .filter_map(|dialect| {
            let primitive = dialect.primitive(name)?;
            (dialect.name_of(primitive) == Some(name)).then_some(match primitive {
                Primitive::CreateSignal => matches!(index, 0 | 1),
                _ => false,
            })
        })
        .collect::<Vec<_>>();
    !answers.is_empty() && answers.into_iter().all(|answer| answer)
}

/// Returns true only when the exact public name is a **props merge** —
/// a call that yields an object carrying its arguments' reactivity — in every
/// dialect that canonically exports it (ADR 0109).
///
/// The dialect-agnostic form of [`Dialect::merges_props_reactivity`], for the
/// certifier: a census reads a producer fact naming a module specifier and an
/// export, and nothing in a transcript says which dialect the artifact resolved.
/// Answering only where the two agree is what lets it ask without choosing.
/// Both dialects answer for their own spelling and are silent about the other's,
/// so `mergeProps` is decided by 1.x alone and `merge` by 2.0 alone, and neither
/// is decided by a dialect that does not export it.
#[must_use]
pub fn unambiguous_props_merge(name: &str) -> bool {
    let answers = DIALECTS
        .iter()
        .copied()
        .filter_map(|dialect| {
            let primitive = dialect.primitive(name)?;
            (dialect.name_of(primitive) == Some(name))
                .then(|| dialect.merges_props_reactivity(primitive))
        })
        .collect::<Vec<_>>();
    !answers.is_empty() && answers.into_iter().all(|answer| answer)
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

/// The reactive role of one slot of what `name` returns, when every dialect
/// that canonically exports `name` agrees.
///
/// `None` means "no dialect answer" and is never "not reactive": a name no
/// dialect exports, a slot no dialect has audited, and a slot the two dialects
/// disagree about all answer `None`, and a caller may read none of them as a
/// negative claim.
///
/// Disagreement includes silence. If one dialect that canonically exports the
/// name answers a role and another answers nothing, the pair has not agreed,
/// so the answer is `None` — the opposite reading would let 1.x's audited
/// vocabulary speak for a 2.0 name whose owner has not reviewed it.
///
/// Kept separate from [`unambiguous_callable_result_tuple_item`] on purpose:
/// that one answers *callability* for the return path, and widening it into a
/// reactivity table would make one answer carry two claims.
#[must_use]
pub fn unambiguous_reactive_result_slot(name: &str, slot: ResultSlot) -> Option<ReactiveRole> {
    let answers = DIALECTS
        .iter()
        .copied()
        .filter_map(|dialect| {
            let primitive = dialect.primitive(name)?;
            (dialect.name_of(primitive) == Some(name))
                .then(|| dialect.reactive_result_slot(primitive, slot))
        })
        .collect::<Vec<_>>();
    let first = *answers.first()?;
    answers
        .into_iter()
        .all(|answer| answer == first)
        .then_some(first)
        .flatten()
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

/// The context role every dialect that states one gives `name`, when they all
/// agree and at least one does ([`Dialect::context_role`]). Silence in one
/// dialect while another answers is disagreement, and answers `None`.
#[must_use]
pub fn unambiguous_context_role(name: &str) -> Option<ContextRole> {
    let answers = DIALECTS
        .iter()
        .map(|dialect| dialect.context_role(name))
        .collect::<Vec<_>>();
    let first = (*answers.first()?)?;
    answers
        .into_iter()
        .all(|answer| answer == Some(first))
        .then_some(first)
}

/// Whether every dialect that canonically exports `name` states that reading
/// the accessor at `slot` of its result runs no code when every argument of the
/// creating call is a primitive by grammar ([`Dialect::inert_accessor_read`],
/// ADR 0146). Silence in any one of them answers `false`, as it does for
/// [`unambiguous_reactive_result_slot`].
#[must_use]
pub fn unambiguous_inert_accessor_read(name: &str, slot: ResultSlot) -> bool {
    let answers = DIALECTS
        .iter()
        .copied()
        .filter_map(|dialect| {
            let primitive = dialect.primitive(name)?;
            (dialect.name_of(primitive) == Some(name))
                .then(|| dialect.inert_accessor_read(primitive, slot))
        })
        .collect::<Vec<_>>();
    !answers.is_empty() && answers.into_iter().all(|answer| answer)
}

/// ADR 0183: whether every dialect that canonically exports `name` states the
/// callback at `argument` of a call with `argument_count` arguments runs
/// eagerly as the compute of a computation the call creates
/// ([`Dialect::eager_owned_computation_slot`]).
#[must_use]
pub fn unambiguous_eager_owned_computation_slot(
    name: &str,
    argument: usize,
    argument_count: usize,
) -> bool {
    let answers = DIALECTS
        .iter()
        .copied()
        .filter_map(|dialect| {
            let primitive = dialect.primitive(name)?;
            (dialect.name_of(primitive) == Some(name))
                .then(|| dialect.eager_owned_computation_slot(primitive, argument, argument_count))
        })
        .collect::<Vec<_>>();
    !answers.is_empty() && answers.into_iter().all(|answer| answer)
}

/// ADR 0183: whether every dialect that canonically exports `name` states the
/// callback at `argument` runs during the call, synchronously and inline
/// ([`Dialect::runs_callback_synchronously`] for the primitive, and an
/// [`Execution::Inline`] row at this slot): `createRoot`'s body, `untrack`'s
/// function, `runWithOwner`'s function.
#[must_use]
pub fn unambiguous_synchronous_callback_slot(
    name: &str,
    argument: usize,
    argument_count: usize,
) -> bool {
    let answers = DIALECTS
        .iter()
        .copied()
        .filter_map(|dialect| {
            let primitive = dialect.primitive(name)?;
            (dialect.name_of(primitive) == Some(name))
                .then(|| dialect.synchronous_callback_slot(primitive, argument, argument_count))
        })
        .collect::<Vec<_>>();
    !answers.is_empty() && answers.into_iter().all(|answer| answer)
}

/// ADR 0180: whether every dialect that canonically exports `name` states its
/// inert read ignores the options argument
/// ([`Dialect::inert_read_ignores_options`]).
#[must_use]
pub fn unambiguous_inert_read_ignores_options(name: &str) -> bool {
    let answers = DIALECTS
        .iter()
        .copied()
        .filter_map(|dialect| {
            let primitive = dialect.primitive(name)?;
            (dialect.name_of(primitive) == Some(name))
                .then(|| dialect.inert_read_ignores_options(primitive))
        })
        .collect::<Vec<_>>();
    !answers.is_empty() && answers.into_iter().all(|answer| answer)
}

/// The argument position every dialect that canonically exports `name` agrees
/// holds its options object ([`Dialect::options_argument`]), or `None` when one
/// is silent or they disagree.
#[must_use]
pub fn unambiguous_options_argument(name: &str) -> Option<usize> {
    let answers = DIALECTS
        .iter()
        .copied()
        .filter_map(|dialect| {
            let primitive = dialect.primitive(name)?;
            (dialect.name_of(primitive) == Some(name)).then(|| dialect.options_argument(primitive))
        })
        .collect::<Vec<_>>();
    let first = (*answers.first()?)?;
    answers
        .into_iter()
        .all(|answer| answer == Some(first))
        .then_some(first)
}

/// Whether every dialect that canonically exports `name` states that invoking
/// the accessor at `slot` of its result is a read of the computation the
/// creating call registered ([`Dialect::computed_accessor_read`], ADR 0162).
/// Silence in any one of them answers `false`.
#[must_use]
pub fn unambiguous_computed_accessor_read(name: &str, slot: ResultSlot) -> bool {
    let answers = DIALECTS
        .iter()
        .copied()
        .filter_map(|dialect| {
            let primitive = dialect.primitive(name)?;
            (dialect.name_of(primitive) == Some(name))
                .then(|| dialect.computed_accessor_read(primitive, slot))
        })
        .collect::<Vec<_>>();
    !answers.is_empty() && answers.into_iter().all(|answer| answer)
}

/// Bind a computed-accessor row to the exact archive its runtime was read in.
/// An audited archive alone does not establish every row about every export.
#[must_use]
pub fn computed_accessor_read_is_audited_for(
    name: &str,
    slot: ResultSlot,
    archive: &AuditedArchive,
) -> bool {
    let answers = DIALECTS
        .iter()
        .copied()
        .filter_map(|dialect| {
            let primitive = dialect.primitive(name)?;
            (dialect.name_of(primitive) == Some(name))
                .then(|| dialect.computed_accessor_read_archive(primitive, slot, archive))
        })
        .collect::<Vec<_>>();
    !answers.is_empty() && answers.into_iter().all(|answer| answer)
}

/// Whether some dialect exports `name` from `origin_module` in value position.
///
/// This is a dialect answer about *where a name can come from*, not a resolved
/// package identity: the module string a producer states is the written import
/// specifier, so this premise says "the specifier is one a Solid dialect
/// exports this name from" and nothing more. An empty module — which is what a
/// locally declared value reports — is `false`, which is the whole point: a
/// package's own `function createSignal()` must never answer a question about
/// the dialect's.
#[must_use]
pub fn exports_value_from(origin_module: &str, name: &str) -> bool {
    if origin_module.is_empty() || name.is_empty() {
        return false;
    }
    DIALECTS.iter().any(|dialect| {
        dialect
            .export_modules(name, ExportPosition::Value)
            .contains(&origin_module)
    })
}

/// Whether `package` is a package whose own published bytes define some
/// dialect's primitives — an exact name match against
/// [`Dialect::primitive_defining_packages`] for either dialect.
///
/// Unioned across dialects deliberately: the question is about the *archive*
/// under analysis, not about which vocabulary a consuming project selected. A
/// 1.x project analyzing `@solidjs/signals` is analyzing bytes that define
/// 2.0's primitives, and the path heuristic that misreads them does not
/// consult the selected dialect either.
///
/// This is a **generation-scope** predicate, not a proof of identity: it
/// compares a name, with no version and no integrity behind it. It may only
/// ever *withhold* a claim (open a domain), never establish one — see ADR 0005.
#[must_use]
pub fn primitive_defining_package(package: &str) -> bool {
    if package.is_empty() {
        return false;
    }
    DIALECTS
        .iter()
        .any(|dialect| dialect.primitive_defining_packages().contains(&package))
}

/// Whether a dependency name places a package inside some carried dialect's
/// ecosystem: a package a dialect defines its primitives in, or anything under
/// a scope that exists because the dialect does.
///
/// Unioned across dialects for the same reason [`primitive_defining_package`]
/// is: the question is about the manifest under inspection, not about which
/// vocabulary the analyzing project selected. Like that predicate it compares
/// a *name*, so it may only widen what is examined, never establish a claim.
#[must_use]
pub fn ecosystem_dependency(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    primitive_defining_package(name)
        || DIALECTS.iter().any(|dialect| {
            dialect
                .ecosystem_scopes()
                .iter()
                .any(|scope| name.starts_with(scope))
        })
}

/// Whether a *specifier* names the built-in runtime foundation — the package
/// itself or a subpath of it.
///
/// Built from [`Dialect::primitive_defining_packages`] rather than from a
/// list of its own: the three names were written out twice, and a fourth
/// core package added to a dialect would have reached one copy and not the
/// other.
#[must_use]
pub fn core_runtime_specifier(specifier: &str) -> bool {
    DIALECTS
        .iter()
        .flat_map(|dialect| dialect.primitive_defining_packages())
        .any(|name| {
            specifier == *name
                || specifier
                    .strip_prefix(name)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        })
}

/// Whether a contract reference must be withheld from ordinary analysis
/// because it names the built-in runtime foundation. This is not a resolver
/// or proof of runtime identity: even an untrusted spelling can only remove
/// contract authority here, never grant built-in semantics.
#[must_use]
pub fn core_runtime_contract_reference(package: &str, specifier: &str) -> bool {
    primitive_defining_package(package) || core_runtime_specifier(specifier)
}

/// One of the eight **kinded** call claim domains a normalized package
/// contract publishes.
///
/// `throws` is the ninth call claim domain and deliberately has no variant
/// here. It is the one domain `validate_call_claims` constrains to no
/// operation kind, so "this export publishes no operation of kind X" has no X
/// for it: its items are operations already published under another kind and
/// additionally labelled as able to complete abruptly
/// (`docs/package-contract-v2/semantic-model.md` § throws). A negative table
/// keyed on operation kind cannot say anything about it, and the
/// implementation-census plan § 4.5 records that `throws` is not a census
/// target under version 1 at all.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallClaimDomain {
    Callbacks,
    Reads,
    Writes,
    Creates,
    Invalidates,
    Returns,
    Cleanups,
    Disposals,
}

impl CallClaimDomain {
    /// The domain's key in a normalized `call` object, and the string that
    /// appears in that object's `closed` list.
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Callbacks => "callbacks",
            Self::Reads => "reads",
            Self::Writes => "writes",
            Self::Creates => "creates",
            Self::Invalidates => "invalidates",
            Self::Returns => "returns",
            Self::Cleanups => "cleanups",
            Self::Disposals => "disposals",
        }
    }

    /// The single `operation.kind` this domain admits.
    #[must_use]
    pub const fn operation_kind(self) -> &'static str {
        match self {
            Self::Callbacks => "invoke",
            Self::Reads => "read",
            Self::Writes => "write",
            Self::Creates => "create",
            Self::Invalidates => "invalidate",
            Self::Returns => "return",
            Self::Cleanups => "cleanup",
            Self::Disposals => "dispose",
        }
    }
}

/// The exact published archive one dialect's negative rows were read against.
///
/// All four fields are the audited document's own `package` block, and all
/// four are part of the identity. A name-and-version tuple is strictly weaker
/// than the evidence on hand and is the first disqualifying objection in
/// `docs/adr/0005-dialect-axioms-about-the-dialects-own-package.md`: any
/// registry serving a self-consistent coordinate would otherwise receive the
/// answer on bytes nobody in this repository ever read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuditedArchive {
    /// `package.name`.
    pub name: &'static str,
    /// `package.version`.
    pub version: &'static str,
    /// `package.integrity` — the Subresource Integrity of the published
    /// tarball, verbatim (`sha512-…`).
    pub integrity: &'static str,
    /// `package.manifest.sha256` — the digest of the archive's own
    /// `package.json`, verbatim and unprefixed.
    pub manifest_sha256: &'static str,
}

/// Exactly which audited bytes one negative row was read from.
///
/// The two variants differ in **what the cited bytes are evidence of**, and the
/// difference is not cosmetic — it decides what a test can mechanically
/// re-establish:
///
/// - [`AuditedCitation::Summary`] cites a normalized contract document. The
///   cited range *is the claim*: it parses as the summary object carrying the
///   domain's empty collection and the `closed` list that closes it, so a test
///   re-reads the range and **re-derives the closure itself**. A row cannot
///   drift from the bytes it cites, and no human judgement sits between the
///   bytes and the row.
/// - [`AuditedCitation::Implementation`] cites the archive's own runtime bytes,
///   read by hand in a repository audit document. The cited range is the
///   *definition a human read*, and closure was the reading's conclusion, not
///   the range's content: no machine here re-derives "this body performs no
///   `create`" from a JavaScript function body. What the digests pin is
///   therefore the **subject** of the review — that the row still cites the
///   same bytes of the same published file that the audit's section walked —
///   and the audit document is where the reasoning is reviewable. Use this
///   variant only where no audited summary exists for the export.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditedCitation {
    /// A summary object inside an audited contract document.
    Summary {
        /// Repository-relative path of the audited document.
        document: &'static str,
        /// The summary's id in that document's `summaries` map.
        summary: &'static str,
        /// First byte of the summary object in the document.
        start_byte: usize,
        /// One past the last byte of the summary object in the document.
        end_byte: usize,
    },
    /// A hand implementation census over the archive's own runtime bytes,
    /// recorded in a repository audit document.
    Implementation {
        /// Repository-relative path of the audit document.
        audit: &'static str,
        /// The section heading of the audit that decides this row, as the
        /// literal heading text, so a reader can find it and a test can assert
        /// it exists.
        section: &'static str,
        /// The runtime file inside the archive, package-relative, exactly as
        /// spelled in the pinned per-file manifest.
        archive_path: &'static str,
        /// `sha256` of that whole file as recorded in the pinned per-file
        /// manifest of the row's own archive,
        /// `benchmarks/package-contract-v2/phase0/<release>/<archive>/files.json`
        /// (`rc3/` for the `2.0.0-rc.3` archives, `rc6/` for
        /// `@solidjs/signals@2.0.0-rc.6`, `rc9/` for the three
        /// `2.0.0-rc.9` archives).
        file_sha256: &'static str,
        /// First byte of the cited definition in that file.
        start_byte: usize,
        /// One past the last byte of the cited definition in that file.
        end_byte: usize,
        /// `sha256` of exactly those bytes, so the range can be re-verified
        /// without trusting the offsets. The same bytes are checked into
        /// `rust/crates/solid-dialect/audited-slices/` so the check runs with
        /// no archive install.
        slice_sha256: &'static str,
    },
}

/// One negative row: the audited contract for this archive publishes **no**
/// operation of this domain's kind for this export.
///
/// Every artifact case and condition of the audited archive that exports the
/// name must close the domain empty, or the row is absent — unless the row is
/// [`RowScope::HostTarget`], which states exactly which condition and which
/// runtime files it was read on and answers nowhere else. Absence is
/// "not modelled", never "no" — see [`primitive_performs_no_operation`].
///
/// # A row is about one archive, not one package
///
/// `(package, version)` names exactly one [`AuditedArchive`] of the same
/// authority, and [`DialectNegativeAuthority::denies`] matches both. A reading
/// of one prerelease's bytes therefore never answers for another prerelease of
/// the same package, however similar the bytes: listing a second archive under
/// a name extends *no* existing row to it, and every row the new archive
/// carries has to be read on its own bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NegativeClaimRow {
    /// Must equal the [`AuditedArchive::name`] of the archive this row is
    /// about; the row is meaningless without the rest of that tuple.
    pub package: &'static str,
    /// Must equal the [`AuditedArchive::version`] of the archive this row was
    /// read against. Together with `package` it names exactly one archive of
    /// the authority; a test pins that.
    pub version: &'static str,
    /// The audited document's own export key, which is also a canonical
    /// primitive spelling of the owning dialect.
    pub export: &'static str,
    pub domain: CallClaimDomain,
    /// Which resolutions of the archive the row answers for. Every row the
    /// table carried before 2026-09-25 is [`RowScope::EveryCondition`], and
    /// only those answer [`DialectNegativeAuthority::denies`] and
    /// [`primitive_performs_no_operation`]; a [`RowScope::HostTarget`] row is
    /// reachable only through [`host_target_row`], whose caller must replay
    /// the scope's premises.
    pub scope: RowScope,
    /// Every document, runtime file, and condition the row was read from.
    /// Never empty, and never a mix of the two [`AuditedCitation`] kinds: a row
    /// has one authority, and a test pins that.
    pub citations: &'static [AuditedCitation],
}

/// Which resolutions of its archive a [`NegativeClaimRow`] answers for.
///
/// # Why a row can be narrower than its archive
///
/// A row is `(package, version, export, domain)`, and the certification tier
/// binds the archive the callee's *declaration* lives in. The runtime a
/// consumer executes is chosen by the `exports` condition, and one archive can
/// ship bodies that differ by condition: `solid-js@2.0.0-rc.3`'s `createSignal`
/// performs no `create` in its browser build and reaches `ctx.serialize` in its
/// `node`/`worker`/`deno` build, which `semantic-model.md` § creates'
/// [Decision 2026-09-04] counts as one. "A guarded reach still counts, and a
/// flat row must withhold" — so the flat row stays withheld, and what the audit
/// *did* establish is stated as a scoped row the census may consult only when
/// it can replay the scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowScope {
    /// The row holds for every runtime file the archive's `exports` map can
    /// select. The only scope [`DialectNegativeAuthority::denies`] reads.
    EveryCondition,
    /// The row holds only where the archive resolves under a host-target
    /// condition to one of the runtime files the audit read, and only when
    /// every call the audit followed out of the archive is answered by an
    /// audited [`RowScope::EveryCondition`] row of the archive it reaches.
    HostTarget(HostTargetScope),
    /// The row holds only for a call whose arguments satisfy
    /// [`ArgumentScope`], which the census checks at the call site, and only
    /// when every call the audit followed out of the archive is answered by an
    /// audited [`RowScope::EveryCondition`] row of the archive it reaches.
    /// Read through [`argument_row`] and nothing else.
    Arguments(ArgumentScope),
}

/// The argument premises a [`RowScope::Arguments`] row rests on (ADR 0168).
///
/// A row's audit is a reading of what the archive's *own* code does at the call
/// event. That reading is silent about a callable the caller hands over: a
/// callable's reads are the caller's, but only when the census can see whose
/// callable it is. Each premise below is checked by the census at the call
/// site, against the producer's facts about the call, and a call that does not
/// satisfy every one of them is not answered by the row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArgumentScope {
    /// Argument slots that must be a primitive by their grammar alone
    /// (`arguments_primitive_syntax`), which no function is. The reading holds
    /// only where the archive's `typeof x === "function"` test fails, so a slot
    /// the census cannot prove is not a function refuses. A slot the call does
    /// not write, or a spread, is not proved.
    pub primitive_slots: &'static [usize],
    /// Argument slots the archive's own code sends down one path when the value
    /// is a function (it invokes it) and another when it is not, and the audit
    /// cleared both: each must be proved a primitive by its grammar, or hold a
    /// callable attributable as for [`Self::invoked_slots`].
    pub callable_or_primitive_slots: &'static [usize],
    /// Argument slots the archive's own code invokes during the call. The
    /// invoked callable's reads are its author's, and the census can attribute
    /// them only when the slot holds one of: a function literal lexically inside
    /// the implementation being walked (its calls are walked with the frame), or
    /// a value rooted at a parameter of that implementation (the caller's own
    /// callable). A reference to a callable declared elsewhere, or a value the
    /// call built, may read a signal this very call created, and refuses.
    pub invoked_slots: &'static [usize],
    /// `(package, export, domain)` answers the reading delegated to: calls the
    /// audit followed into another archive and read there. Each must be denied
    /// by an [`RowScope::EveryCondition`] row of the one authenticated, audited
    /// archive of that package in the certification's closure.
    pub delegates: &'static [(&'static str, &'static str, CallClaimDomain)],
}

/// The premises a [`RowScope::HostTarget`] row rests on. Each is checked by
/// the census terminator against authenticated bytes, never assumed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostTargetScope {
    /// The host-target export condition that must be in the certification's
    /// requested set. A consumer that declared no host never receives a case
    /// certified under it (`contract_interface::admissible_cases`).
    pub condition: HostTargetCondition,
    /// The archive-relative runtime files the audit read, exactly as spelled in
    /// the pinned per-file manifest. The archive's `.` entry, resolved under
    /// exactly the requested condition set, must select one of them; a file the
    /// audit did not walk refuses even when the condition matches.
    pub runtime: &'static [&'static str],
    /// `(package, export, domain)` answers the reading delegated to: calls the
    /// audit followed into another archive and read there. Each must be denied
    /// by an [`RowScope::EveryCondition`] row of the one authenticated,
    /// audited archive of that package in the certification's closure.
    pub delegates: &'static [(&'static str, &'static str, CallClaimDomain)],
}

/// A host-target export condition: the runtime a package is certified for and
/// a consumer runs in, and the condition a [`HostTargetScope`] may name.
///
/// The four are the resolver's mutually exclusive host axis
/// (`MUTUALLY_EXCLUSIVE_CONDITION_AXES` in `packages/cli/scripts/
/// artifact-resolution.mjs`). Package certification runs once per host it
/// certifies for (ADR 0140: `browser` and `node`), and a case carrying one of
/// these is a claim about that host's runtime bodies only.
///
/// An enum rather than a string so that adding a host is a compile error at
/// every match, including the consumer's admission rule: a host that declared
/// none of these never receives a case carrying one, and a host that declared
/// one receives only cases certified under exactly its hosts
/// (`contract_interface::admissible_cases`). Only [`Self::Browser`] scopes a
/// negative row today; `node`, `deno` and `worker` carry none.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum HostTargetCondition {
    Browser,
    Node,
    Deno,
    Worker,
}

impl HostTargetCondition {
    /// Every host-target condition.
    pub const ALL: [Self; 4] = [Self::Browser, Self::Node, Self::Deno, Self::Worker];

    /// The export-condition spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Browser => "browser",
            Self::Node => "node",
            Self::Deno => "deno",
            Self::Worker => "worker",
        }
    }

    /// The host-target condition spelled `condition`, if it is one.
    #[must_use]
    pub fn from_condition(condition: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|host| host.as_str() == condition)
    }

    /// Whether `condition` is the spelling of some host-target condition. A
    /// consumer host that declared no conditions never receives an artifact
    /// case carrying one.
    #[must_use]
    pub fn names(condition: &str) -> bool {
        Self::from_condition(condition).is_some()
    }
}

/// One dialect's negative authority: which archives it audited, and what those
/// audits deny.
///
/// A **negative** authority and nothing else. It can refuse to answer and it
/// can answer "the audit publishes no operation of this kind"; it can never
/// answer that an operation exists, and it may not be read as closing a domain
/// on the audited package's *own* certification — see the module-level rules on
/// [`primitive_performs_no_operation`].
#[derive(Clone, Copy, Debug)]
pub struct DialectNegativeAuthority {
    /// The archives whose audited documents the rows were read from. An
    /// archive with no row is still listed when it was read: that is what makes
    /// "this dialect audited these bytes and found nothing to deny" different
    /// from "this dialect never looked".
    pub archives: &'static [AuditedArchive],
    /// Sorted by `(package, version, export, domain)`; a test pins the order
    /// and the absence of duplicates.
    pub rows: &'static [NegativeClaimRow],
}

impl DialectNegativeAuthority {
    /// The archives in this authority named `name`.
    pub fn archives_named(&self, name: &str) -> impl Iterator<Item = &'static AuditedArchive> {
        self.archives
            .iter()
            .filter(move |archive| archive.name == name)
    }

    /// Whether this authority lists exactly `archive` and carries the row for
    /// `(archive, export, domain)`.
    ///
    /// The row must name the archive's **name and version**: a row read on one
    /// prerelease never answers for another prerelease of the same package.
    /// An archive this authority does not list — including one that agrees on
    /// name and version but not on integrity or manifest digest — denies
    /// nothing.
    ///
    /// Only a [`RowScope::EveryCondition`] row answers here. A
    /// [`RowScope::HostTarget`] row is a narrower claim whose premises the
    /// caller must replay, so it is reachable only through
    /// [`Self::host_target`].
    #[must_use]
    pub fn denies(&self, archive: &AuditedArchive, export: &str, domain: CallClaimDomain) -> bool {
        self.archives.contains(archive)
            && self.rows.iter().any(|row| {
                row.package == archive.name
                    && row.version == archive.version
                    && row.export == export
                    && row.domain == domain
                    && row.scope == RowScope::EveryCondition
            })
    }

    /// The [`RowScope::Arguments`] scope this authority states for exactly
    /// `(archive, export, domain)`, when it lists `archive` and carries such a
    /// row. `None` otherwise, including where the row is
    /// [`RowScope::EveryCondition`] (ask [`Self::denies`]).
    #[must_use]
    pub fn arguments(
        &self,
        archive: &AuditedArchive,
        export: &str,
        domain: CallClaimDomain,
    ) -> Option<&'static ArgumentScope> {
        if !self.archives.contains(archive) {
            return None;
        }
        self.rows.iter().find_map(|row| match &row.scope {
            RowScope::Arguments(scope)
                if row.package == archive.name
                    && row.version == archive.version
                    && row.export == export
                    && row.domain == domain =>
            {
                Some(scope)
            }
            _ => None,
        })
    }

    /// The [`RowScope::HostTarget`] scope this authority states for exactly
    /// `(archive, export, domain)`, when it lists `archive` and carries such a
    /// row. `None` otherwise, including where the row is
    /// [`RowScope::EveryCondition`] (ask [`Self::denies`]).
    #[must_use]
    pub fn host_target(
        &self,
        archive: &AuditedArchive,
        export: &str,
        domain: CallClaimDomain,
    ) -> Option<&'static HostTargetScope> {
        if !self.archives.contains(archive) {
            return None;
        }
        self.rows.iter().find_map(|row| match &row.scope {
            RowScope::HostTarget(scope)
                if row.package == archive.name
                    && row.version == archive.version
                    && row.export == export
                    && row.domain == domain =>
            {
                Some(scope)
            }
            _ => None,
        })
    }
}

/// The audited archives named `name`, across both dialects.
///
/// Returns the identity tuples a caller must compare *field by field* against
/// an authenticated snapshot before consulting
/// [`primitive_performs_no_operation`]. The comparison is deliberately the
/// caller's: it is the side that can name which field disagreed, exactly as
/// the lock replay in `contract_certification/dependencies.rs` does.
///
/// Empty means no dialect audited an archive under this name, which refuses
/// everything downstream.
#[must_use]
pub fn audited_archives(name: &str) -> Vec<&'static AuditedArchive> {
    if name.is_empty() {
        return Vec::new();
    }
    DIALECTS
        .iter()
        .flat_map(|dialect| dialect.negative_claim_authority().archives_named(name))
        .collect()
}

/// The public entry points of the package a dialect's dependency tracking
/// lives in, by exact package name and exact export name (ADR 0163).
///
/// A synthesized `reads: []` veto runs the export under test as the compute of
/// a fresh memo, created by these entry points under a fresh root, and observes
/// whether that memo gained a dependency. Nothing here names a private field:
/// the module finds the node's dependency fields at run time by comparing a
/// memo that read a signal with one that read nothing, so the fields a build
/// mangles are the build's own answer, never a constant of this crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackingRuntime {
    /// The package whose bytes implement tracking: an exact npm name.
    pub package: &'static str,
    /// Runs a callback under a fresh owner, handing it a disposer.
    pub create_root: &'static str,
    /// Creates a computation whose compute runs, tracked, when it is created.
    pub create_memo: &'static str,
    /// Creates a readable source: `[read, write]`.
    pub create_signal: &'static str,
    /// The computation currently tracking reads, or `null`.
    pub get_observer: &'static str,
}

/// The tracking runtime some carried dialect ships in `package`, if any.
#[must_use]
pub fn tracking_runtime(package: &str) -> Option<&'static TrackingRuntime> {
    if package.is_empty() {
        return None;
    }
    DIALECTS
        .iter()
        .filter_map(|dialect| dialect.tracking_runtime())
        .find(|runtime| runtime.package == package)
}

/// Every tracking runtime a carried dialect ships.
pub fn tracking_runtimes() -> impl Iterator<Item = &'static TrackingRuntime> {
    DIALECTS
        .iter()
        .filter_map(|dialect| dialect.tracking_runtime())
}

/// Whether `name` is a canonical primitive spelling of some dialect — the name
/// the dialect's own table round-trips, not an alias and not a near miss.
#[must_use]
pub fn canonical_primitive_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    DIALECTS.iter().any(|dialect| {
        dialect
            .primitive(name)
            .is_some_and(|primitive| dialect.name_of(primitive) == Some(name))
    })
}

/// Whether **some** dialect's negative authority carries a row denying
/// `domain` for `package`'s export spelled `export`, with no archive identity
/// bound.
///
/// # The package is half the question
///
/// A row is `(package, export, domain)`, and an export name is not an identity.
/// 2.0's `solid-js` re-*declares* `createSignal`, `createMemo`, `createStore`,
/// `createProjection`, `createOptimistic` and `createOptimisticStore` from
/// `./client/hydration.js` rather than re-exporting `@solidjs/signals`', and
/// [`Solid2`]'s audit withholds rows for those six implementations on purpose
/// (`solid_2.rs` § 7.4 — the `node`/`worker`/`deno` bodies reach
/// `ctx.serialize`, which *is* a create). Matching on the name alone answered
/// all six out of `@solidjs/signals`' rows: it proposed a closed `creates` the
/// audit refuses to make, and emitted no decline record to say which primitive
/// still needs auditing.
///
/// **`package` must be the package that declares the callee**, resolved from
/// the declaration's source file — not the module its import specifier named.
/// The two differ exactly where this matters: measured against the audited rc.3
/// install, a `solid-js` import reports `solid-js` as its origin module for
/// `untrack` and `createRoot` as readily as for `createSignal`, though the
/// first two are re-exports of `@solidjs/signals`' own declarations. Keying on
/// the specifier would decline those ten legitimate re-exports to fix these six
/// re-declarations.
///
/// An empty `package` denies nothing. That is the safe polarity for the one
/// caller: silence means *do not propose*, so a callee whose declaration did
/// not resolve keeps its domain open rather than closing it on a guess.
///
/// # This answers a proposal question, never a proof one
///
/// [`primitive_performs_no_operation`] is the proof-bearing form: it takes an
/// [`AuditedArchive`] tuple the caller has already bound field by field against
/// an authenticated snapshot, and it is the only form a census terminator may
/// consult. This one takes a bare **name**, so it establishes nothing about
/// which bytes a call actually reached — a hoisted sibling installation, a
/// user's own `createSignal`, and the audited archive all answer the same here.
///
/// It exists for exactly one caller: the generator deciding whether to *propose*
/// a closed `creates` domain for a consuming package's export. A proposal is an
/// unaccepted claim that the certifier's implementation census then has to prove
/// against authenticated bytes (`docs/adr/0008-implementation-census-for-creates.md`),
/// so a name-level read here can only make the generator propose something the
/// census may refuse — it can never certify anything. The generator's use is
/// also polarity-correct: silence means *do not propose*, so an unaudited or
/// withheld primitive keeps the domain open rather than closing it.
///
/// It is version-blind as well as integrity-blind: a row read on *any* audited
/// archive of `package` answers here. That is the same proposal-only latitude,
/// and it is why this function must never grow a proof-bearing caller — the
/// archive-scoped answer is [`primitive_performs_no_operation`]'s.
///
/// # The case's conditions decide whether a scoped row is consulted
///
/// `conditions` is the export-condition set of the artifact case being
/// proposed. A [`RowScope::HostTarget`] row answers only when its condition is
/// in that set, so an `["import"]` case never proposes a closure that only a
/// `["browser","import"]` certification could discharge — the census would
/// withhold it, and the proposal would have cost a certification pass to learn
/// nothing. Everything else the census checks about the scope (the resolved
/// runtime file, the delegated archive) is left to the census: this function
/// stays name-level.
#[must_use]
pub fn some_audit_denies_primitive(
    package: &str,
    export: &str,
    domain: CallClaimDomain,
    conditions: &std::collections::BTreeSet<String>,
) -> bool {
    if package.is_empty() || export.is_empty() || !canonical_primitive_name(export) {
        return false;
    }
    DIALECTS.iter().any(|dialect| {
        let authority = dialect.negative_claim_authority();
        authority.rows.iter().any(|row| {
            row.package == package
                && row.export == export
                && row.domain == domain
                && match &row.scope {
                    RowScope::EveryCondition => true,
                    RowScope::HostTarget(scope) => conditions.contains(scope.condition.as_str()),
                    // A proposal carries no call site to check the arguments
                    // against, so it never answers from one.
                    RowScope::Arguments(_) => false,
                }
        })
    })
}

/// The [`RowScope::HostTarget`] scope every dialect listing `archive` states
/// for `(archive, export, domain)`, or `None`.
///
/// The census terminator's entry to a scoped row, and only that. `Some` is
/// **not** an answer: it hands the caller the premises it must replay against
/// authenticated bytes — the requested condition set, the runtime file the
/// archive resolves to under exactly that set, and each delegated archive's own
/// [`primitive_performs_no_operation`] answer. Cross-dialect agreement is the
/// same as [`primitive_performs_no_operation`]'s: every authority listing the
/// exact tuple must state the identical scope, and one authority's silence is
/// silence.
#[must_use]
pub fn host_target_row(
    archive: &AuditedArchive,
    export: &str,
    domain: CallClaimDomain,
) -> Option<&'static HostTargetScope> {
    if export.is_empty() || !canonical_primitive_name(export) {
        return None;
    }
    let mut answers = DIALECTS.iter().filter_map(|dialect| {
        let authority = dialect.negative_claim_authority();
        authority
            .archives
            .contains(archive)
            .then(|| authority.host_target(archive, export, domain))
    });
    let first = answers.next()??;
    answers.all(|other| other == Some(first)).then_some(first)
}

/// The [`RowScope::Arguments`] scope every dialect listing `archive` states for
/// `(archive, export, domain)`, or `None` (ADR 0168).
///
/// Like [`host_target_row`], `Some` is **not** an answer: it hands the census
/// the premises it must replay at the call site and against authenticated
/// bytes. Cross-dialect agreement is the same as
/// [`primitive_performs_no_operation`]'s.
#[must_use]
pub fn argument_row(
    archive: &AuditedArchive,
    export: &str,
    domain: CallClaimDomain,
) -> Option<&'static ArgumentScope> {
    if export.is_empty() || !canonical_primitive_name(export) {
        return None;
    }
    let mut answers = DIALECTS.iter().filter_map(|dialect| {
        let authority = dialect.negative_claim_authority();
        authority
            .archives
            .contains(archive)
            .then(|| authority.arguments(archive, export, domain))
    });
    let first = answers.next()??;
    answers.all(|other| other == Some(first)).then_some(first)
}

/// Whether the audited contract for `archive` publishes **no** operation of
/// `domain`'s kind for `export`.
///
/// # This is a negative authority, and only a census terminator
///
/// A `true` answer restates an audit: some dialect read the published bytes of
/// this exact archive and its audited document closes `domain` empty for
/// `export` in every artifact case and condition that exports it. It is
/// admissible in exactly one place — as a **terminator** in an implementation
/// census of some *other* package's export, where the question is "can this
/// resolved callee perform the domain's operation" (see
/// `docs/package-contract-v2/phase21/2026-09-03-implementation-census-plan.md`
/// § 3.1).
///
/// It is **not** admissible to close a domain on the audited package's own
/// certification. That is the positive dialect axiom
/// `docs/adr/0005-dialect-axioms-about-the-dialects-own-package.md` defers, and
/// its objection 5 is fatal there: the demand and the discharge would come from
/// the same dialect rows. Applied to a callee the objection does not arise,
/// because the axiom discharges nothing about the export under certification —
/// see ADR 0007.
///
/// # Silence is never "no"
///
/// `false` means "this table does not deny it" and never "the export performs
/// the operation". An archive no dialect audited, an export whose audit leaves
/// the domain open or nonempty in *any* condition, an export deliberately
/// withheld, and a domain no dialect has admitted yet all answer `false`.
///
/// # Cross-dialect agreement is keyed by the archive, not its name
///
/// `solid-js` is an archive **name** both dialects own, at different versions.
/// The caller has already bound the exact archive tuple — name, version,
/// integrity and manifest digest — field by field against
/// [`audited_archives`]; `archive` carries that binding. An authority
/// participates in the union below only when its own `archives` list contains
/// this *exact* tuple, never merely a same-named one: two dialects auditing
/// different bytes under one name never interact, whatever either says about
/// that name. Among the authorities that do list this archive, the answer is
/// `true` only when every one of them carries the row; disagreement —
/// including one authority's silence — is silence.
///
/// Within one authority the same holds between archives: a row names its
/// archive's version as well as its name ([`NegativeClaimRow::version`]), so
/// `@solidjs/signals@2.0.0-rc.3`'s rows say nothing about
/// `@solidjs/signals@2.0.0-rc.6`'s bytes, which carry their own, and rc.6's
/// say nothing about rc.9's.
#[must_use]
pub fn primitive_performs_no_operation(
    archive: &AuditedArchive,
    export: &str,
    domain: CallClaimDomain,
) -> bool {
    if export.is_empty() || !canonical_primitive_name(export) {
        return false;
    }
    let answers = DIALECTS
        .iter()
        .filter_map(|dialect| {
            let authority = dialect.negative_claim_authority();
            authority
                .archives
                .contains(archive)
                .then(|| authority.denies(archive, export, domain))
        })
        .collect::<Vec<_>>();
    !answers.is_empty() && answers.into_iter().all(|denied| denied)
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
    /// with, other than the language's own vocabulary. Listed so a variant
    /// can be found again by its [`Dialect::variant_key`] alone, which is what
    /// a daemon forwarding the selection it hashed has.
    fn variants(&self) -> &'static [&'static dyn Dialect] {
        &[]
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
    /// Contract *generation* consults this to withhold the domains that
    /// recognition would otherwise fabricate. The `--check-contracts` report
    /// also reads it, for the adjacent question of which packages a dialect
    /// ships itself; that is the same set by construction, and a build where
    /// the two disagreed would be describing an archive it does not model. No
    /// diagnostic reads it.
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

    /// Where this dialect's dependency tracking lives, for the synthesized
    /// `reads: []` veto (ADR 0163). `None` -- the default -- synthesizes no such
    /// veto: a dialect that has not stated which package tracks reads, and
    /// through which entry points, leaves every `reads: []` candidate withheld
    /// for want of a recipe, which is what it was before the veto existed.
    fn tracking_runtime(&self) -> Option<&'static TrackingRuntime> {
        None
    }

    /// Identity of the reviewed built-in runtime model. Facts cite
    /// `builtin-solid://<identity>#<primitive>`, never a package receipt.
    /// Increment the model revision when the authority or interpretation
    /// changes; Solid 1 and Solid 2 have independent identities.
    fn runtime_model_identity(&self) -> &'static str;

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
    /// ADR 0183: whether the callback at `argument` of a call with
    /// `argument_count` arguments runs during the call, synchronously and
    /// inline: the primitive [runs its callback synchronously](Self::runs_callback_synchronously)
    /// and this slot's row is [`Execution::Inline`].
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

    /// Whether the callback at `argument` runs during the call, directly
    /// under a **root** owner the call creates for it, so that the ambient
    /// owner a write guard reads there is that root.
    ///
    /// A root is a children-capable owner, so a write guard that exempts only
    /// children-forbidden leaves rejects a write there. Nothing about the
    /// callback's *reads* follows from this: a root body is untracked. The
    /// default claims nothing, which leaves such a write to the ordinary
    /// callback classification.
    fn callback_runs_in_created_root(&self, primitive: Primitive, argument: usize) -> bool {
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
    /// Core return classification is owned here, without a package-contract
    /// overlay. For tuple positions use [`Dialect::reactive_result_slot`];
    /// this answer alone cannot distinguish a store from its setter.
    fn returns_store(&self, primitive: Primitive) -> bool;

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
    /// and stays absent until this dialect's owner has one, which is why the
    /// aggregate [`unambiguous_reactive_result_slot`] treats one dialect's
    /// silence as disagreement rather than deferring to the other's row.
    fn reactive_result_slot(&self, primitive: Primitive, slot: ResultSlot) -> Option<ReactiveRole> {
        let _ = (primitive, slot);
        None
    }

    /// Whether invoking the accessor at `slot` of what `primitive` returns runs
    /// no code at all -- not the caller's, not the package's, not a callback's
    /// -- when **the creating call's first argument cannot be a function and
    /// its options argument, if any, carries no callback** (ADR 0146, and ADR
    /// 0162 for what discharges it): a read that observes the source's current
    /// value and does nothing else. `false` is the silence every unaudited row
    /// keeps.
    ///
    /// The precondition is the dialect's to state because it is what makes the
    /// answer true: a callable first argument, or an options object whose
    /// callbacks the source keeps, is exactly how a read comes to run code.
    /// The census discharges it at the call site from the producer's grammar
    /// facts: each argument a primitive by grammar; or the first argument an
    /// array or object literal (`typeof first !== "function"` is the only test
    /// the audited bodies make of it); or the options slot
    /// ([`unambiguous_options_argument`]) an object literal of primitives.
    fn inert_accessor_read(&self, primitive: Primitive, slot: ResultSlot) -> bool {
        let _ = (primitive, slot);
        false
    }

    /// ADR 0180: whether the inert read [`Dialect::inert_accessor_read`]
    /// states for `primitive` is the same whatever its options argument holds.
    ///
    /// The inert row describes what invoking the accessor does. A runtime
    /// whose read never consults the options object -- its callbacks run on
    /// the setter and on unlink, which the read claim does not describe --
    /// answers `true`, and the census then discharges the options slot of a
    /// non-spread call by position alone. The default `false` keeps the
    /// original condition: the options argument is an object literal of
    /// primitives.
    fn inert_read_ignores_options(&self, primitive: Primitive) -> bool {
        let _ = primitive;
        false
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

    /// Whether invoking the accessor at `slot` of what `primitive` returns is a
    /// read of a computation the creating call registered (ADR 0162): it
    /// observes the computation's current value in the invoking caller's
    /// tracking context, and when that value is stale it re-runs **the
    /// registered computation itself**, which is the code the creating call
    /// was handed and whose executions are the *creating* call's `creates` and
    /// `callbacks` claims, not an invocation of the read. It invokes no other
    /// callable. `false` is the silence every unaudited row keeps.
    ///
    /// The computation is accounted where it is registered. Additional
    /// options callbacks are not covered: certification requires undisplaced
    /// non-spread arguments and callback-free options by grammar.
    fn computed_accessor_read(&self, primitive: Primitive, slot: ResultSlot) -> bool {
        let _ = (primitive, slot);
        false
    }

    /// The exact audited archive that supports the computed-accessor row.
    /// Metadata without a runtime audit cannot certify a returned value.
    fn computed_accessor_read_archive(
        &self,
        primitive: Primitive,
        slot: ResultSlot,
        archive: &AuditedArchive,
    ) -> bool {
        let _ = (primitive, slot, archive);
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

    /// Whether the runtime never invokes callback `slot` of `primitive` while
    /// the call runs, on every path: it only stores or returns the callable.
    /// Stronger than [`Execution::Deferred`], which is a tracking attribution
    /// and usually, but not always, means later than the call. A call's own
    /// `reads` cannot depend on such a callback. Unknown is `false`.
    fn callback_never_invoked_during_call(&self, _primitive: Primitive, _slot: usize) -> bool {
        false
    }

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

    /// The **package-contract** word for a callback at this slot, which is a
    /// different question from [`Dialect::callback_execution_at`].
    ///
    /// That one answers attribution for the checker's own analysis: whose reads
    /// a callback subscribes. This one answers what a published contract
    /// promises a consumer about *observable scheduling relative to the
    /// exported call*. The two agree for most primitives and deliberately
    /// diverge for some — Solid 1.x's `createResource` fetcher is `Deferred`
    /// for attribution (it runs outside the source computation) and `Inline`
    /// for a contract (both overloads invoke it before `createResource`
    /// returns), and 1.x's `on` returns an adapter whose callbacks run at the
    /// eventual invocation rather than during the creating call.
    ///
    /// `None` means this dialect states no contract word for the slot, which
    /// contract emission must treat as "unknown" rather than as any of the
    /// three answers.
    ///
    /// This lived as a hardcoded `match` in `solid-reactive-ir`'s
    /// `interproc.rs` until ADR 0111. It belongs here: it is per-version
    /// behaviour, and shared code holding it meant a new dialect could not
    /// state a different answer.
    fn contract_callback_execution_at(
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
                && !self.runs_callback_deferred(primitive),
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
    /// [`Dialect::contract_callback_execution_at`] carry that difference.
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
        match primitive {
            Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateTrackedEffect => Some(OwnerRequirementRole::Effect),
            Primitive::OnCleanup => Some(OwnerRequirementRole::Cleanup),
            Primitive::OnSettled => Some(OwnerRequirementRole::SettledCleanup),
            _ => None,
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

    /// The member a context must be accessed through to act as a provider,
    /// if this dialect has one. Solid 1.x exposes `.Provider`, whose value
    /// getter runs untracked; Solid 2.0 makes the context itself the
    /// provider, so there is no member to name.
    fn context_provider_member(&self) -> Option<&'static str> {
        None
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

    /// What this dialect's audits of its own packages **deny**, per exact
    /// archive identity, per canonical export, per call claim domain.
    ///
    /// The one table in this crate whose rows are read out of the checked-in
    /// audited contracts rather than out of a runtime probe, and the one whose
    /// rows cite a byte range so a test can re-derive them. It answers a
    /// question about a *callee* — never about the archive under
    /// certification. See [`primitive_performs_no_operation`] for the
    /// admissibility rules and ADR 0007 for why the callee side is sound where
    /// the self side is not.
    fn negative_claim_authority(&self) -> &'static DialectNegativeAuthority;
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

    /// Every vocabulary this build carries: each dialect, and each release
    /// variant it can answer an installation with. The whole-table invariants
    /// below hold for every one of them, not only for the audited vocabulary.
    fn dialects() -> &'static [&'static dyn Dialect] {
        static EVERY: std::sync::LazyLock<Vec<&'static dyn Dialect>> =
            std::sync::LazyLock::new(|| {
                DIALECTS
                    .iter()
                    .flat_map(|dialect| {
                        std::iter::once(*dialect).chain(dialect.variants().iter().copied())
                    })
                    .collect()
            });
        &EVERY
    }

    /// The single archive some dialect audited as `name@version`, for tests
    /// that need the bound tuple `primitive_performs_no_operation` now takes
    /// rather than a bare name.
    fn audited_archive(name: &str, version: &str) -> AuditedArchive {
        let archives = audited_archives(name)
            .into_iter()
            .filter(|archive| archive.version == version)
            .collect::<Vec<_>>();
        assert_eq!(
            archives.len(),
            1,
            "expected exactly one archive audited as {name}@{version}"
        );
        *archives[0]
    }

    /// An archive tuple no dialect's authority lists, for tests that need to
    /// pass a bound archive that must never match.
    const fn unaudited_archive(name: &'static str) -> AuditedArchive {
        AuditedArchive {
            name,
            version: "0.0.0-unaudited",
            integrity: "sha512-unaudited",
            manifest_sha256: "unaudited",
        }
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
            (&Solid2 as &dyn Dialect).type_role("solid-js", "Signal"),
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
    fn contract_schema_exemptions(version: Version, name: &str) -> bool {
        match version {
            // `Version::V1` survives for classification only; no vocabulary
            // behind it means no contract bundle and no name to exempt, and
            // the loop below never reaches this arm. Kept exhaustive so the
            // next major has to state its own exemptions here.
            Version::V1 => false,
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
        for (version, bundle, packages) in
            [(Version::V2, "solid-v2", &["solid-js", "@solidjs/web"][..])]
        {
            let dialect = &Solid2 as &dyn Dialect;
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
        //
        // The floor was 20 while the 1.x bundle was also cross-checked, and
        // measuring it after the retirement is how we learned that **most of
        // that came from 1.x**: the Solid 2 bundle's own contract files carry
        // callback rows for only four modelled primitives. The guard is
        // re-pinned to the real 2.0 number rather than deleted, and closing
        // that gap is contract work, not dialect work.
        assert!(checked >= 4, "only {checked} callbacks cross-checked");
    }

    #[test]
    fn every_recognized_name_round_trips() {
        for &dialect in dialects() {
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
        let two = &Solid2 as &dyn Dialect;

        // 1.x: createEffect(fn, value?) — the callback is first, and index 1 is
        // a seed VALUE. 2.0: createEffect(compute, apply) — index 1 is the
        // apply callback. Reading 1.x's seed as a callback is the single
        // highest-yield way to get this wrong.
        assert_eq!(two.callback_positions(Primitive::CreateEffect), &[1]);
        assert_eq!(two.callback_positions(Primitive::CreateRenderEffect), &[1]);

        // Unchanged across the split.
        assert_eq!(two.callback_positions(Primitive::CreateMemo), &[0]);
        assert_eq!(two.callback_positions(Primitive::Untrack), &[0]);
    }

    #[test]
    fn deferred_execution_is_independent_of_callback_position() {
        let two = &Solid2 as &dyn Dialect;

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
            // flush(fn) runs fn() inline and touches neither the owner nor the
            // listener: a read inside it subscribes the caller (probed on
            // rc.0-rc.9, `Solid2::runs_callback_deferred`).
            Primitive::Flush,
        ] {
            assert!(!two.runs_callback_deferred(primitive), "{primitive:?}");
        }
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
        let two = &Solid2 as &dyn Dialect;

        // `createRevealOrder` is here for the same reason `createRoot` is — it
        // clears tracking while establishing an owner and runs its callback
        // immediately. `flush` is not: `@solidjs/signals` `flush(fn)` runs
        // `fn()` inside a `try { return fn() } finally { flush(); syncDepth-- }`,
        // invoked during the call but with the caller's listener still current,
        // so a read inside it subscribes the caller (probed on rc.0-rc.9). It
        // is inline and transparent, as `latest` and `isPending` are.
        assert_eq!(
            synchronous_clearing_names(two),
            vec!["createRevealOrder", "createRoot", "runWithOwner", "untrack"]
        );
        assert!(!two.runs_callback_synchronously(Primitive::Flush));
        // ADR 0183: the slot form names each of them at its callback slot only.
        assert!(unambiguous_synchronous_callback_slot("createRoot", 0, 1));
        assert!(unambiguous_synchronous_callback_slot("untrack", 0, 1));
        assert!(unambiguous_synchronous_callback_slot("runWithOwner", 1, 2));
        assert!(!unambiguous_synchronous_callback_slot("runWithOwner", 0, 2));
        assert!(!unambiguous_synchronous_callback_slot("flush", 0, 1));
        assert!(!unambiguous_synchronous_callback_slot("createMemo", 0, 1));

        // The two halves of `runs_callback_deferred` stay separable: a
        // genuinely later callback is never synchronous, and a primitive the
        // dialect models no callback for is never either.
        for primitive in [
            Primitive::OnCleanup,
            Primitive::CreateReaction,
            Primitive::OnSettled,
            Primitive::Action,
            Primitive::Lazy,
        ] {
            assert!(!two.runs_callback_synchronously(primitive), "{primitive:?}");
        }
        // Inline but not listener-clearing: `batch` is transparent to its call
        // site, so it is not in this set either.
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
        let two = &Solid2 as &dyn Dialect;

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

        assert_eq!(
            two.tracked_callback_timing(Primitive::CreateEffect, 1, 2),
            None
        );
        assert_eq!(two.tracked_callback_timing(Primitive::Untrack, 0, 1), None);
    }

    #[test]
    fn the_async_boundary_is_a_role_not_a_name() {
        let two = &Solid2 as &dyn Dialect;

        assert_eq!(two.boundary_kind("Loading"), Some(Boundary::Async));

        // Each dialect refuses the other's spelling.
        assert_eq!(two.boundary_kind("Suspense"), None);
    }

    #[test]
    fn names_removed_in_solid_2_are_not_recognized() {
        let two = &Solid2 as &dyn Dialect;
        for name in ["batch", "createComputed", "createResource", "Suspense"] {
            assert_eq!(two.primitive(name), None, "{name} is gone in Solid 2.0");
        }
    }

    /// A dialect that states only what the trait *requires*.
    ///
    /// The 26 methods with no default, each answering "nothing": no modules,
    /// no primitives, no boundaries, no packages, no audited archives. It is
    /// never registered and never analyzes anything. Its whole job is to be
    /// asked the 36 *defaulted* questions, so the test below can say what a
    /// second dialect gets for free before it has decided anything.
    ///
    /// This is the cheap half of the second-dialect proof. `ALL`, the
    /// `Version` variant and the compiler adapter are compile-time decisions
    /// whose omission fails the build (`docs/adding-a-dialect.md`); a default
    /// that silently hands 2.0's answer to a dialect that never stated one
    /// fails nothing at all, which is why it is worth a test.
    struct Silent;

    impl Dialect for Silent {
        fn version(&self) -> Version {
            // Arbitrary and unused: `Silent` is never registered, and nothing
            // here resolves a dialect by version.
            Version::V1
        }
        fn modules(&self) -> &'static [&'static str] {
            &[]
        }
        fn primitive_defining_packages(&self) -> &'static [&'static str] {
            &[]
        }
        fn ecosystem_scopes(&self) -> &'static [&'static str] {
            &[]
        }
        fn runtime_model_identity(&self) -> &'static str {
            "silent/model-0"
        }
        fn primitive(&self, _name: &str) -> Option<Primitive> {
            None
        }
        fn name_of(&self, _primitive: Primitive) -> Option<&'static str> {
            None
        }
        fn callback_positions(&self, _primitive: Primitive) -> &'static [usize] {
            &[]
        }
        fn runs_callback_deferred(&self, _primitive: Primitive) -> bool {
            false
        }
        fn boundary_kind(&self, _tag: &str) -> Option<Boundary> {
            None
        }
        fn boundary_name(&self, _boundary: Boundary) -> &'static str {
            ""
        }
        fn cleanup_rule(&self, _primitive: Primitive) -> CleanupRule {
            CleanupRule::Never
        }
        fn accepts_cleanup_return(&self, _primitive: Primitive) -> bool {
            false
        }
        fn renders_children_through_callback(&self, _primitive: Primitive) -> bool {
            false
        }
        fn creates_reactive_source(&self, _primitive: Primitive) -> bool {
            false
        }
        fn creates_directive_owner(&self, _primitive: Primitive) -> bool {
            false
        }
        fn merges_props_reactivity(&self, _primitive: Primitive) -> bool {
            false
        }
        fn splits_props(&self, _primitive: Primitive) -> bool {
            false
        }
        fn returns_reactive_tuple(&self, _primitive: Primitive) -> bool {
            false
        }
        fn children_accessor_parameters(
            &self,
            _primitive: Primitive,
            _key: KeyForm,
        ) -> &'static [usize] {
            &[]
        }
        fn returns_store(&self, _primitive: Primitive) -> bool {
            false
        }
        fn options_argument(&self, _primitive: Primitive) -> Option<usize> {
            None
        }
        fn callback_executions(&self, _primitive: Primitive) -> &'static [(usize, Execution)] {
            &[]
        }
        fn export_modules(&self, _name: &str, _position: ExportPosition) -> Vec<&'static str> {
            Vec::new()
        }
        fn namespace_import_primitives(&self, _module: &str) -> &'static [&'static str] {
            &[]
        }
        fn negative_claim_authority(&self) -> &'static DialectNegativeAuthority {
            static NONE: DialectNegativeAuthority = DialectNegativeAuthority {
                archives: &[],
                rows: &[],
            };
            &NONE
        }
    }

    /// What a dialect that has decided nothing is taken to have said.
    ///
    /// Every assertion here is the *conservative* side of its question, and
    /// the comment says what the other side would have claimed. A default that
    /// drifts to the convenient answer is the failure this catches: the next
    /// dialect would inherit a claim about its runtime that nobody made.
    #[test]
    fn a_dialect_that_states_nothing_claims_nothing() {
        let silent = &Silent as &dyn Dialect;

        // Phase and scheduling: no apply slot, no deferred-callback role, no
        // tracked-read reporting. Inheriting 2.0's `Some(1)` would name a
        // phase in a language that may thread a seed value through that slot.
        assert_eq!(
            silent.apply_callback_argument(Primitive::CreateEffect),
            None
        );
        assert_eq!(
            silent.callback_execution_at(Primitive::CreateEffect, 1, 2),
            None
        );
        assert!(!silent.reports_untracked_reads_at(Primitive::CreateReaction, 0, 1));
        assert!(silent.callback_owners(Primitive::CreateEffect).is_empty());

        // Ownership and writes: a leaf scope forbids writes, no primitive
        // preserves the owner write context, no store root is readonly. Each
        // `true` would be a permission granted without evidence.
        assert!(!silent.leaf_scopes_allow_writes());
        assert!(!silent.callback_preserves_owner_write_context(Primitive::CreateEffect));
        assert!(!silent.leaf_owner_requires_owned_call_site(Primitive::OnCleanup));
        // No component body is a root, so no root exemption reaches one.
        assert!(!silent.component_body_runs_under_root());
        // No owner edge for a host scheduler's callback: 2.0's `None` would
        // prove unowned every callback a language with task-carried owners
        // runs owned.
        assert_eq!(silent.fresh_stack_callback_owner(), None);
        assert!(!silent.store_root_properties_are_readonly());
        assert!(!silent.store_setter_callback_enables_proxy_writes());
        // No release is audited for a dialect that names none: the notice,
        // not silence, under the language's own vocabulary.
        assert!(silent.release_owners().is_empty());
        assert!(silent.audited_installation().is_empty());
        let InstallationReview::Analyzed { vocabulary, gaps } =
            silent.review_installation(&[InstalledRelease {
                package: "solid-js",
                version: Some("2.0.0-rc.3"),
            }])
        else {
            panic!("a dialect that names no release refuses none");
        };
        assert!(vocabulary.is_none());
        assert_eq!(gaps.len(), 1, "{gaps:?}");
        assert_eq!(silent.variant_key(), None);
        assert!(silent.variants().is_empty());
        assert!(silent.defaulted_vocabulary().is_none());

        // Surfaces this dialect has not claimed to model.
        assert!(!silent.models_server_functions());
        assert!(!silent.props_require_caller_proof());
        assert!(!silent.reports_member_reads_after_await());
        assert_eq!(silent.context_provider_member(), None);
        assert!(!silent.static_event_values_are_attributes());
        assert!(!silent.false_attribute_value_removes_attribute());
        assert!(!silent.computation_read_is_render(Primitive::CreateRenderEffect));
        assert!(!silent.mounts_component_tree(Primitive::Render));
        assert!(!silent.direct_jsx_return_is_component());
        assert!(!silent.component_name_may_be_component("Button"));
        // No tracking runtime: inheriting 2.0's `@solidjs/signals` would gate
        // this dialect's `reads: []` candidates on another language's graph.
        assert_eq!(silent.tracking_runtime(), None);

        // The derived questions follow the required answers rather than a
        // second table: a dialect with no modules owns none, a dialect with no
        // primitives declares none, a dialect with no boundaries opens none.
        assert!(!silent.owns_module("solid-js"));
        assert!(!silent.declares_primitive("createEffect"));
        assert!(!silent.is_async_boundary("Loading"));
        assert_eq!(silent.type_role("solid-js", "Accessor"), None);

        // The one default that is *not* "nothing", stated here so it is a
        // decision rather than an oversight: a dialect that declares
        // `Primitive::CreateEffect` inherits the owner-requirement partition
        // every dialect so far has agreed on. It only applies to primitives
        // the dialect actually declares -- `Silent` maps no name to any
        // primitive, so nothing in a real analysis ever reaches it.
        assert_eq!(
            silent.owner_requirement_role(Primitive::CreateEffect),
            Some(OwnerRequirementRole::Effect)
        );
        assert_eq!(
            silent.owner_requirement_role(Primitive::OnCleanup),
            Some(OwnerRequirementRole::Cleanup)
        );
        assert_eq!(silent.owner_requirement_role(Primitive::CreateSignal), None);
    }

    #[test]
    fn a_resolved_solid_js_version_picks_its_dialect() {
        use Classification::{Modelled, UnmodelledMajor};
        assert_eq!(Version::for_solid_js("1.9.14"), Some(Modelled(Version::V1)));
        // 2.0 is still a prerelease; refusing to classify the RC would leave
        // every current 2.0 project on the caller's fallback.
        assert_eq!(
            Version::for_solid_js("2.0.0-rc.0"),
            Some(Modelled(Version::V2))
        );
        assert_eq!(Version::for_solid_js("^1.8.0"), Some(Modelled(Version::V1)));
        assert_eq!(Version::for_solid_js("v2.0.0"), Some(Modelled(Version::V2)));
        // A major nobody here names is still a major, and saying so is the
        // whole point: `None` would send an installed Solid 3 to the caller's
        // 2.0 default, which is the silent-wrong-language outcome ADR 0110
        // closed for 1.x. Major 0 classifies the same way -- `solid-js@0.x`
        // shipped, and a placeholder `0.0.0` stub is a contradicted answer
        // about a real install rather than an absence.
        assert_eq!(Version::for_solid_js("3.0.0"), Some(UnmodelledMajor(3)));
        assert_eq!(Version::for_solid_js("0.5.0"), Some(UnmodelledMajor(0)));
        // Not a version at all: the caller falls back, as it always has.
        assert_eq!(Version::for_solid_js("workspace:*"), None);
        assert_eq!(Version::for_solid_js(""), None);
    }

    #[test]
    fn the_boundary_name_round_trips_through_the_boundary_kind() {
        for &dialect in dialects() {
            let version = dialect.version();
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
            (&Solid2 as &dyn Dialect).boundary_name(Boundary::Async),
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
        for &dialect in dialects() {
            let version = dialect.version();
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

        // 2.0 folded the store and DOM APIs into core and has no subpath to
        // fold them out of; 1.x kept them under `solid-js/store`.
        let two = &Solid2 as &dyn Dialect;
        assert_eq!(
            two.export_modules("createStore", ExportPosition::Value),
            ["solid-js"]
        );

        // A name the dialect does not export has no module, which is not the
        // same answer as "the package root". This is where the old fallback
        // was wrong: it answered `solid-js` for everything it had no arm for.
        assert!(
            two.export_modules("createResource", ExportPosition::Value)
                .is_empty()
        );
    }

    /// The per-argument questions are independent, and the 2.0 answer to each
    /// is pinned here.
    ///
    /// This was a differential against the 1.x vocabulary, and every pair in
    /// it had been a single hardcoded list in the engine holding 2.0's answer.
    /// The contrast is gone with the 1.x dialect; the 2.0 half is kept in
    /// full, because these are exactly the API shapes the checker must not
    /// drift from — `createEffect(compute, apply)`, `createMemo(compute,
    /// options)`, `createStore(value, options)`.
    #[test]
    fn the_argument_questions_are_independent_and_pinned_to_the_two_zero_api() {
        let two = &Solid2 as &dyn Dialect;

        // createEffect takes a tracked compute arm and a deferred apply arm.
        // 1.x's second argument was a seed, and a read in it was once reported
        // as running in an "apply callback" 1.x did not have.
        assert_eq!(
            two.callback_executions(Primitive::CreateEffect),
            [(0, Execution::Tracked), (1, Execution::Deferred)]
        );

        // createMemo is `(compute, options?)` — 1.x's positional `value`
        // parameter is gone, so the options slot moved from 2 to 1.
        assert_eq!(two.options_argument(Primitive::CreateMemo), Some(1));
        assert!(two.supports_sync_option(Primitive::CreateMemo));
        // createStore keeps a value parameter, so its options slot is 2.
        assert_eq!(two.options_argument(Primitive::CreateStore), Some(2));
        // The store family has an options slot but no `sync` routing: rc.0
        // rebuilds projection node options with only `loadingValue`/`name`,
        // so `sync: true` is inert on all three constructors (probed).
        assert!(!two.supports_sync_option(Primitive::CreateStore));
        assert!(!two.supports_sync_option(Primitive::CreateProjection));
        assert!(!two.supports_sync_option(Primitive::CreateOptimisticStore));

        // createComputed does not exist in 2.0 at all.
        assert!(
            two.callback_executions(Primitive::CreateComputed)
                .is_empty()
        );

        // Stores: createStore and the projection pair are 2.0's; createMutable
        // was 1.x's and 2.0 does not have it.
        assert!(two.returns_store(Primitive::CreateStore));
        assert!(two.returns_store(Primitive::CreateProjection));
        assert!(!two.returns_store(Primitive::CreateMutable));
    }

    /// An options index that is also a callback position means the engine
    /// would read a function as an options object, or the reverse.
    #[test]
    fn no_primitive_takes_its_options_where_it_takes_a_callback() {
        for &dialect in dialects() {
            let version = dialect.version();
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
        for &dialect in dialects() {
            let version = dialect.version();
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
        // Unmodelled is not ownerless: a caller must not read an empty answer
        // as "creates no owner".
        assert!(
            (&Solid2 as &dyn Dialect)
                .callback_owners(Primitive::Children)
                .is_empty()
        );

        // 2.0 splits the effect into a compute arm that creates an owner and
        // an apply arm. `createEffect` queues the apply, so it always runs
        // unowned; `createRenderEffect` runs its first apply during the call,
        // under the caller's owner, and only its later runs unowned.
        assert_eq!(
            (&Solid2 as &dyn Dialect).callback_owners(Primitive::CreateEffect),
            &[(0, CallbackOwner::Creates), (1, CallbackOwner::None)]
        );
        assert_eq!(
            (&Solid2 as &dyn Dialect).callback_owners(Primitive::CreateRenderEffect),
            &[
                (0, CallbackOwner::Creates),
                (1, CallbackOwner::InheritsFirstRun)
            ]
        );

        // Both signatures accept Owner | null. A concrete call sharpens this
        // flat answer from its first argument.
        for &dialect in dialects() {
            assert_eq!(
                dialect.callback_owners(Primitive::RunWithOwner),
                &[(1, CallbackOwner::Conditional)]
            );
        }

        // resolve(fn) wraps its thunk in createRoot, which its signature does
        // not suggest.
        assert_eq!(
            (&Solid2 as &dyn Dialect).callback_owners(Primitive::Resolve),
            &[(0, CallbackOwner::Creates)]
        );
    }

    /// A declaration site and a namespace import ask different questions, and
    /// sharing one list made the narrower answer govern both. Every name in
    /// the vocabulary must be recognisable where it is declared, or adding it
    /// to the table accomplishes nothing.
    #[test]
    fn every_name_in_the_vocabulary_is_recognisable_at_its_declaration() {
        for &dialect in dialects() {
            let version = dialect.version();
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
        let two = &Solid2 as &dyn Dialect;
        assert!(two.declares_primitive("latest"));
        assert!(
            two.namespace_import_primitives("solid-js")
                .contains(&"latest")
        );
    }

    #[test]
    fn concrete_callback_contracts_cover_overloads_and_both_dialects() {
        let two = &Solid2 as &dyn Dialect;

        assert_eq!(
            two.callback_execution_at(Primitive::CreateSignal, 0, 1),
            Some(Execution::Tracked)
        );
        assert_eq!(
            two.callback_execution_at(Primitive::CreateEffect, 1, 2),
            Some(Execution::Deferred)
        );

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
    /// The props-merging primitive is one behaviour under two spellings, and
    /// the engine used to ask for 2.0's. Nothing else in the vocabulary merges
    /// props, so a second `true` here would be a claim about a primitive whose
    /// result is not a props root.
    fn each_dialect_names_its_own_props_merge() {
        let two = &Solid2 as &dyn Dialect;

        assert!(two.merges_props_reactivity(Primitive::Merge));
        // The other dialect's spelling is not a second answer: each dialect
        // answers for its own vocabulary and is silent about the other's.
        assert!(!two.merges_props_reactivity(Primitive::MergeProps));

        for &dialect in dialects() {
            let merging = dialect_names(dialect)
                .iter()
                .filter_map(|name| dialect.primitive(name))
                .filter(|primitive| dialect.merges_props_reactivity(*primitive))
                .map(|primitive| format!("{primitive:?}"))
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(
                merging.len(),
                1,
                "exactly one name in each vocabulary returns a props object carrying its \
                 arguments' reactivity, got {merging:?}"
            );
            // A splitting primitive returns a *tuple* of proxies, so the root
            // travels through array destructuring and not through the call's
            // own value.
            for primitive in [Primitive::SplitProps, Primitive::Omit] {
                assert!(!dialect.merges_props_reactivity(primitive));
            }
        }
    }

    #[test]
    /// The other two lists that survived the dialect extraction as literals.
    /// Both were single-vocabulary and shared code asked them of everyone.
    fn each_dialect_names_its_own_props_split_and_tuple_returns() {
        let two = &Solid2 as &dyn Dialect;

        // 1.x's `splitProps` is 2.0's `omit`; neither answers for the other.
        assert!(two.splits_props(Primitive::Omit));
        assert!(!two.splits_props(Primitive::SplitProps));

        // Shared by both: the two-slot returns every dialect has.
        for primitive in [Primitive::CreateSignal, Primitive::CreateStore] {
            assert!(two.returns_reactive_tuple(primitive));
        }
        // 1.x-only, and the member of the old hardcoded list that made it
        // wrong for 2.0.
        assert!(!two.returns_reactive_tuple(Primitive::CreateResource));
        // 2.0-only, and what the old list was missing.
        for primitive in [
            Primitive::CreateOptimistic,
            Primitive::CreateOptimisticStore,
        ] {
            assert!(two.returns_reactive_tuple(primitive));
        }
        // A store returned *whole* is not a tuple, however store-kinded.
        assert!(two.returns_store(Primitive::CreateProjection));
        assert!(!two.returns_reactive_tuple(Primitive::CreateProjection));
        // Every tuple row is a source factory; the converse does not hold.
        for &dialect in dialects() {
            for name in dialect_names(dialect) {
                let Some(primitive) = dialect.primitive(name) else {
                    continue;
                };
                assert!(
                    !dialect.returns_reactive_tuple(primitive)
                        || dialect.creates_reactive_source(primitive),
                    "{name} returns a reactive tuple but is not a source factory"
                );
            }
        }
    }

    #[test]
    fn each_dialect_knows_its_own_reactive_source_factories() {
        let two = &Solid2 as &dyn Dialect;

        for primitive in [
            Primitive::CreateSignal,
            Primitive::CreateMemo,
            Primitive::CreateStore,
        ] {
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
                !two.creates_reactive_source(primitive),
                "{primitive:?} is not 2.0 vocabulary at all"
            );
        }

        // Returns nothing, so it is not a source however reactive it is.
        // 2.0-only factories.
        for primitive in [Primitive::CreateProjection, Primitive::CreateOptimistic] {
            assert!(two.creates_reactive_source(primitive));
        }
    }

    #[test]
    fn conditional_cleanup_rules_survive_the_extraction() {
        let two = &Solid2 as &dyn Dialect;
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
        for &dialect in dialects() {
            let version = dialect.version();
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
        let two = &Solid2 as &dyn Dialect;

        // 1.x splits stores and DOM into subpaths; importing createStore from
        // "solid-js" is wrong there and right in 2.0.
        assert!(!two.owns_module("solid-js/store"));
        assert!(two.owns_module("@solidjs/web"));
        assert!(two.owns_module("solid-js"));
    }

    /// Four of the five control-flow components were shared with 1.x; the
    /// fifth was the point of the old differential. What matters now is the
    /// 2.0 list itself, because a function written inside one of these is a
    /// callback, and reading the wrong list means reading it as a component.
    #[test]
    fn control_flow_components_render_children_through_a_callback() {
        let two = &Solid2 as &dyn Dialect;
        for primitive in [
            Primitive::For,
            Primitive::Show,
            Primitive::Match,
            Primitive::Switch,
            // `Repeat` is 2.0's fifth; 1.x had `Index` here instead.
            Primitive::Repeat,
        ] {
            assert!(two.renders_children_through_callback(primitive));
        }
        assert!(!two.renders_children_through_callback(Primitive::Index));

        // Boundaries render children directly, not through a callback.
        assert!(!two.renders_children_through_callback(Primitive::Loading));
    }

    /// The aggregate answers a result slot only from rows that exist.
    #[test]
    fn reactive_result_slots_answer_only_where_a_row_exists() {
        let two = &Solid2 as &dyn Dialect;
        assert_eq!(
            two.reactive_result_slot(Primitive::CreateSignal, ResultSlot::TupleItem(0)),
            Some(ReactiveRole::Accessor)
        );
        assert_eq!(
            two.reactive_result_slot(Primitive::CreateStore, ResultSlot::TupleItem(0)),
            None,
            "a store slot has no accessor row"
        );
    }

    /// ADR 0146: only the plain signal's accessor is inert, and only its slot.
    #[test]
    fn only_the_plain_signal_accessor_read_is_inert() {
        assert!(unambiguous_inert_accessor_read(
            "createSignal",
            ResultSlot::TupleItem(0)
        ));
        for (name, slot) in [
            ("createSignal", ResultSlot::TupleItem(1)),
            ("createMemo", ResultSlot::Whole),
            ("createStore", ResultSlot::TupleItem(0)),
            ("createOptimistic", ResultSlot::TupleItem(0)),
            ("notADialectName", ResultSlot::TupleItem(0)),
        ] {
            assert!(!unambiguous_inert_accessor_read(name, slot), "{name}");
        }
    }

    /// ADR 0162: the options argument of `createSignal` and `createMemo` is at
    /// index 1 in the audited dialect, and a name no dialect exports has none.
    #[test]
    fn the_options_argument_is_the_dialects_own() {
        assert_eq!(unambiguous_options_argument("createSignal"), Some(1));
        assert_eq!(unambiguous_options_argument("createMemo"), Some(1));
        assert_eq!(unambiguous_options_argument("createStore"), Some(2));
        assert_eq!(unambiguous_options_argument("onCleanup"), None);
        assert_eq!(unambiguous_options_argument("notADialectName"), None);
    }

    /// ADR 0162: `createMemo`'s whole result is a computed read and never an
    /// inert one -- a memo's read is not inert. ADR 0175: `createSignal`'s
    /// accessor is in both rows; the inert one is the stronger answer, asked
    /// first, and the computed one is what a possibly-callable first argument
    /// leaves. Nothing else is computed.
    #[test]
    fn only_a_plain_signal_read_ignores_its_options() {
        assert!(unambiguous_inert_read_ignores_options("createSignal"));
        for name in [
            "createMemo",
            "createStore",
            "createOptimistic",
            "notADialectName",
        ] {
            assert!(!unambiguous_inert_read_ignores_options(name), "{name}");
        }
    }

    #[test]
    fn only_the_memo_and_signal_accessor_reads_are_computed() {
        assert!(unambiguous_computed_accessor_read(
            "createMemo",
            ResultSlot::Whole
        ));
        assert!(!unambiguous_inert_accessor_read(
            "createMemo",
            ResultSlot::Whole
        ));
        assert!(unambiguous_computed_accessor_read(
            "createSignal",
            ResultSlot::TupleItem(0)
        ));
        assert!(unambiguous_inert_accessor_read(
            "createSignal",
            ResultSlot::TupleItem(0)
        ));
        for (name, slot) in [
            ("createMemo", ResultSlot::TupleItem(0)),
            ("createSignal", ResultSlot::Whole),
            ("createSignal", ResultSlot::TupleItem(1)),
            ("createStore", ResultSlot::TupleItem(0)),
            ("createOptimistic", ResultSlot::TupleItem(0)),
            ("notADialectName", ResultSlot::Whole),
        ] {
            assert!(!unambiguous_computed_accessor_read(name, slot), "{name}");
        }
    }

    #[test]
    fn the_computed_accessor_read_binds_only_its_audited_archive() {
        for name in ["@solidjs/signals", "solid-js", "@solidjs/web"] {
            for archive in audited_archives(name) {
                // ADR 0175: the signals archive and `solid-js`' own six builds,
                // both at rc.9, and nothing else -- not `@solidjs/web`, not an
                // older release.
                // ADR 0197 adds rc.13, re-read on its own bytes.
                let audited = matches!(archive.name, "@solidjs/signals" | "solid-js")
                    && matches!(archive.version, "2.0.0-rc.9" | "2.0.0-rc.13");
                assert_eq!(
                    computed_accessor_read_is_audited_for("createMemo", ResultSlot::Whole, archive,),
                    audited,
                    "{}@{}",
                    archive.name,
                    archive.version,
                );
                assert_eq!(
                    computed_accessor_read_is_audited_for(
                        "createSignal",
                        ResultSlot::TupleItem(0),
                        archive,
                    ),
                    audited,
                    "{}@{}",
                    archive.name,
                    archive.version,
                );
                assert!(!computed_accessor_read_is_audited_for(
                    "createSignal",
                    ResultSlot::TupleItem(1),
                    archive,
                ));
                let changed = AuditedArchive {
                    integrity: "sha512-other",
                    ..*archive
                };
                assert!(!computed_accessor_read_is_audited_for(
                    "createMemo",
                    ResultSlot::Whole,
                    &changed,
                ));
            }
        }
    }

    /// The audited archive answers for the exact bytes it names, and a
    /// withdrawn row answers `false` for its own reason.
    #[test]
    fn an_audited_archive_answers_for_its_exact_bytes() {
        let solid_js = *audited_archives("solid-js")
            .into_iter()
            .find(|archive| archive.version == "2.0.0-rc.3")
            .expect("2.0 audits solid-js@2.0.0-rc.3");
        assert!(primitive_performs_no_operation(
            &solid_js,
            "For",
            CallClaimDomain::Creates
        ));
        // `Show`'s row was withdrawn on 2026-09-27 for the same kind of reach:
        // its server body's own memo hands a thenable to `ctx.serialize`.
        assert!(!primitive_performs_no_operation(
            &solid_js,
            "Show",
            CallClaimDomain::Creates
        ));
        // `createEffect`'s row was withdrawn on 2026-09-04 -- its
        // `node`-condition body reaches the SSR serializer and a flat row
        // cannot carry that guard -- so it answers `false` on its own merits.
        assert!(!primitive_performs_no_operation(
            &solid_js,
            "createEffect",
            CallClaimDomain::Creates
        ));
    }

    fn dialect_names(dialect: &'static dyn Dialect) -> Vec<&'static str> {
        match dialect.version() {
            // `Version::V1` survives for classification only; no vocabulary
            // behind it means no name list, and `DIALECTS` never yields one.
            Version::V1 => Vec::new(),
            // A name only some installations export (`until`) is listed for
            // the vocabularies that export it; every other row is listed for
            // all of them, so a row that fails to resolve still fails.
            Version::V2 => solid_2::names()
                .into_iter()
                .filter(|name| {
                    !solid_2::RELEASE_GATED_NAMES.contains(name)
                        || dialect.primitive(name).is_some()
                })
                .collect(),
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
    fn implementation_roles_require_canonical_vocabulary_agreement() {
        assert!(unambiguous_callback_argument("createMemo", 0, 1));
        // `createEffect`'s second argument is unambiguously the apply callback
        // now that 2.0 is the only vocabulary. It was *ambiguous* while 1.x
        // was compiled in, because 1.x's second argument is a seed value --
        // the disagreement was the whole reason this predicate exists, and it
        // will be load-bearing again the moment a second vocabulary returns.
        assert!(unambiguous_callback_argument("createEffect", 1, 2));
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

    // The reactive result table answers a *slot*, which is the whole reason it
    // exists: `unambiguous_callable_result_tuple_item` says both `createSignal`
    // slots are callable, and a proof that read it for reactivity would certify
    // the setter as an accessor.
    #[test]
    fn reactive_result_slots_are_per_slot_and_need_cross_dialect_agreement() {
        assert_eq!(
            unambiguous_reactive_result_slot("createSignal", ResultSlot::TupleItem(0)),
            Some(ReactiveRole::Accessor)
        );
        assert_eq!(
            unambiguous_reactive_result_slot("createSignal", ResultSlot::TupleItem(1)),
            Some(ReactiveRole::Setter)
        );
        assert_eq!(
            unambiguous_reactive_result_slot("createMemo", ResultSlot::Whole),
            Some(ReactiveRole::Accessor)
        );

        // Silence is not a role. A slot past the audited tuple, the whole of a
        // value whose slots are audited, a slot of a value whose whole is, a
        // primitive with no row at all, and a name no dialect exports all
        // answer `None` — and none of those answers may be read as "not
        // reactive".
        assert_eq!(
            unambiguous_reactive_result_slot("createSignal", ResultSlot::TupleItem(2)),
            None
        );
        assert_eq!(
            unambiguous_reactive_result_slot("createSignal", ResultSlot::Whole),
            None
        );
        assert_eq!(
            unambiguous_reactive_result_slot("createMemo", ResultSlot::TupleItem(0)),
            None
        );
        assert_eq!(
            unambiguous_reactive_result_slot("createEffect", ResultSlot::Whole),
            None
        );
        assert_eq!(
            unambiguous_reactive_result_slot("createUnknown", ResultSlot::TupleItem(0)),
            None
        );

        // A name only one dialect canonically exports is answered by that
        // dialect alone; a store result is not an accessor result in either.
        assert_eq!(
            unambiguous_reactive_result_slot("createMutable", ResultSlot::Whole),
            None
        );
        assert_eq!(
            unambiguous_reactive_result_slot("createProjection", ResultSlot::Whole),
            None
        );
    }

    // The module premise replaces a `== "solid-js"` literal in the certifier.
    // Its whole point is the empty module: a package's own locally declared
    // `createSignal` reports no module, and must never answer a question about
    // the dialect's.
    #[test]
    fn value_export_modules_answer_only_for_audited_dialect_modules() {
        assert!(exports_value_from("solid-js", "createSignal"));
        assert!(exports_value_from("solid-js", "createMemo"));
        // 2.0 folded the store API into core, so `createStore` is exported
        // from `solid-js` and the 1.x `solid-js/store` subpath is not a module
        // any compiled vocabulary owns.
        assert!(exports_value_from("solid-js", "createStore"));
        assert!(!exports_value_from("solid-js/store", "createStore"));
        assert!(!exports_value_from("", "createSignal"));
        assert!(!exports_value_from("solid-js", ""));
        assert!(!exports_value_from("my-signals", "createSignal"));
        assert!(!exports_value_from("solid-js", "createUnknown"));
        // A type-position-only export is not a value export.
        assert!(!exports_value_from("solid-js", "Accessor"));
    }

    // Exact names, unioned across dialects, and no subpath or prefix reach:
    // the predicate names *archives*, and a package that merely lives under
    // the `@solidjs` scope is a consumer of these three, not one of them.
    #[test]
    fn primitive_defining_packages_are_exact_archive_names() {
        assert!(primitive_defining_package("solid-js"));
        assert!(primitive_defining_package("@solidjs/signals"));
        assert!(primitive_defining_package("@solidjs/web"));
        assert!(!primitive_defining_package(""));
        assert!(!primitive_defining_package("solid-js/store"));
        assert!(!primitive_defining_package("solid-js/web"));
        assert!(!primitive_defining_package("@solidjs"));
        assert!(!primitive_defining_package("@solidjs/router"));
        assert!(!primitive_defining_package("@solidjs/meta"));
        assert!(!primitive_defining_package("@solidjs/start"));
        assert!(!primitive_defining_package("@solidjs/element"));
        assert!(!primitive_defining_package("solid-js-signals"));
        assert!(!primitive_defining_package("my-solid-js"));
    }

    /// ADR 0163: the tracking runtime is one of the dialect's own archives,
    /// found by exact name only, and every release of it is audited -- a
    /// synthesized `reads: []` veto runs only over an audited copy.
    #[test]
    fn the_tracking_runtime_is_an_audited_primitive_defining_archive() {
        let runtime = tracking_runtime("@solidjs/signals").expect("Solid 2 states one");
        assert!(primitive_defining_package(runtime.package));
        assert!(!audited_archives(runtime.package).is_empty());
        assert_eq!(tracking_runtimes().count(), 1);
        for name in [
            "",
            "solid-js",
            "@solidjs/web",
            "@solidjs/signals/",
            "signals",
        ] {
            assert_eq!(tracking_runtime(name), None, "{name:?}");
        }
    }

    /// The specifier form reaches subpaths, which the archive-name form must
    /// not, and it is derived from the same list rather than repeating it.
    ///
    /// This predicate has a use that the archive-name one is documented never
    /// to have: `module_closure::record_external` uses it to *not* record an
    /// opaque frontier. Its admissibility is argued at that call site — the
    /// exemption establishes no claim, it only declines to withdraw every
    /// domain — and pinned here so the two forms cannot drift into answering
    /// differently about the same package.
    #[test]
    fn the_core_runtime_specifier_form_reaches_subpaths_of_the_same_archives() {
        for name in ["solid-js", "@solidjs/signals", "@solidjs/web"] {
            assert!(primitive_defining_package(name), "{name}");
            assert!(core_runtime_specifier(name), "{name}");
            assert!(
                core_runtime_specifier(&format!("{name}/store")),
                "{name}/store"
            );
        }
        // The archive-name form refuses a subpath; the specifier form is the
        // one that must accept it.
        assert!(!primitive_defining_package("solid-js/store"));
        assert!(core_runtime_specifier("solid-js/store"));

        // Everything the archive-name form refuses outright, this refuses too:
        // a neighbouring scope member, a prefix that is not a path boundary,
        // and the empty specifier.
        for other in [
            "",
            "@solidjs",
            "@solidjs/router",
            "@solidjs/meta",
            "solid-js-signals",
            "my-solid-js",
            "solid-jsx",
        ] {
            assert!(!core_runtime_specifier(other), "{other:?}");
        }
    }

    /// A denial belongs to one package and does not travel by export name.
    ///
    /// The six below are why. 2.0's `solid-js` re-*declares* them from
    /// `./client/hydration.js`, and their rows are withheld for that
    /// implementation on purpose (`solid_2.rs` § 7.4). Answering them out of
    /// `@solidjs/signals`' rows proposed a closed `creates` the audit refuses
    /// to make and swallowed the decline record naming the primitive to audit
    /// next.
    #[test]
    fn a_denial_belongs_to_one_package_and_does_not_travel_by_name() {
        // An `["import"]` case, which is every case the tier holds.
        let none = std::collections::BTreeSet::new();
        for export in [
            "createSignal",
            "createMemo",
            "createStore",
            "createProjection",
            "createOptimistic",
            "createOptimisticStore",
        ] {
            assert!(
                some_audit_denies_primitive(
                    "@solidjs/signals",
                    export,
                    CallClaimDomain::Creates,
                    &none
                ),
                "{export} is denied for the package whose row it is"
            );
            assert!(
                !some_audit_denies_primitive("solid-js", export, CallClaimDomain::Creates, &none),
                "solid-js re-declares {export}, and that implementation has no row"
            );
        }
        // A `["browser","import"]` case: `createSignal` (§ 7.3) and, since
        // 2026-09-28, `createMemo` (rc.9 only) carry a row scoped to
        // `browser`; the four others still have none.
        let browser = ["browser", "import"]
            .into_iter()
            .map(str::to_owned)
            .collect::<std::collections::BTreeSet<_>>();
        for (export, scoped) in [
            ("createSignal", true),
            ("createMemo", true),
            ("createStore", false),
            ("createProjection", false),
            ("createOptimistic", false),
            ("createOptimisticStore", false),
        ] {
            assert_eq!(
                some_audit_denies_primitive("solid-js", export, CallClaimDomain::Creates, &browser),
                scoped,
                "solid-js {export} under a browser case"
            );
        }

        // The other side of the same distinction: `solid-js` re-*exports* these
        // from `@solidjs/signals`, so a callee's declaration resolves into that
        // package and reaches its rows. Nothing here should make a re-export
        // look like a re-declaration.
        for export in ["untrack", "createRoot", "onCleanup", "flush"] {
            assert!(
                some_audit_denies_primitive(
                    "@solidjs/signals",
                    export,
                    CallClaimDomain::Creates,
                    &none
                ),
                "{export} keeps its row"
            );
        }

        // Not knowing the package must not close a domain, and a package with
        // no rows denies nothing.
        assert!(!some_audit_denies_primitive(
            "",
            "createSignal",
            CallClaimDomain::Creates,
            &none
        ));
        assert!(!some_audit_denies_primitive(
            "@solid-primitives/scheduled",
            "createSignal",
            CallClaimDomain::Creates,
            &none
        ));
    }

    /// The 2026-09-23 owner-and-context audit's three rows, each on the package
    /// that declares the callee a `solid-js` import resolves to.
    ///
    /// `runWithOwner` is `@solidjs/signals`' and `solid-js` re-exports it.
    /// `createContext` and `useContext` are `solid-js`' own: `@solidjs/signals`
    /// declares a `createContext` of its own too, which `solid-js` does not
    /// re-export and which nobody has read, so the row must not answer for it.
    /// Each row is `creates` alone.
    #[test]
    fn the_owner_and_context_rows_deny_creates_for_their_declaring_package_only() {
        let none = std::collections::BTreeSet::new();
        for (package, export, other) in [
            ("@solidjs/signals", "runWithOwner", "solid-js"),
            ("solid-js", "createContext", "@solidjs/signals"),
            ("solid-js", "useContext", "@solidjs/signals"),
        ] {
            assert!(
                some_audit_denies_primitive(package, export, CallClaimDomain::Creates, &none),
                "{package} {export}"
            );
            assert!(
                !some_audit_denies_primitive(other, export, CallClaimDomain::Creates, &none),
                "{other} {export}: the denial does not travel by name"
            );
            for domain in [
                CallClaimDomain::Callbacks,
                CallClaimDomain::Reads,
                CallClaimDomain::Writes,
            ] {
                // The 2026-09-30 rc.9 reads audit (ADR 0168) decided `useContext`
                // `reads` as well, under its own audit and section.
                let decided =
                    (package, export, domain) == ("solid-js", "useContext", CallClaimDomain::Reads);
                assert_eq!(
                    some_audit_denies_primitive(package, export, domain, &none),
                    decided,
                    "{package} {export} {domain:?}: the audit decided creates only"
                );
            }
        }
    }

    /// The negative authority is a *negative* authority: it can refuse and it
    /// can deny, and there is no shape of input that makes it assert an
    /// operation exists.
    #[test]
    fn the_negative_authority_answers_only_denials_and_silence() {
        let signals = audited_archive("@solidjs/signals", "2.0.0-rc.3");
        let web = audited_archive("@solidjs/web", "2.0.0-rc.3");

        // A denial the 2.0 audit carries.
        assert!(primitive_performs_no_operation(
            &signals,
            "createTrackedEffect",
            CallClaimDomain::Creates
        ));
        // `render` publishes the operation; `hydrate` is withheld.
        assert!(!primitive_performs_no_operation(
            &web,
            "render",
            CallClaimDomain::Creates
        ));
        assert!(!primitive_performs_no_operation(
            &web,
            "hydrate",
            CallClaimDomain::Creates
        ));
        // A denial whose authority is a hand implementation census over the
        // archive's own runtime bytes rather than an audited summary — the
        // second `AuditedCitation` kind, answered identically from here,
        // because the citation is review material and never a runtime input.
        assert!(primitive_performs_no_operation(
            &signals,
            "createSignal",
            CallClaimDomain::Creates
        ));
        // Real 2.0 export, audited archive, neither an audited summary nor a
        // hand census.
        assert!(!primitive_performs_no_operation(
            &signals,
            "createContext",
            CallClaimDomain::Creates
        ));
        // `reads` is admitted for this export as of 2026-09-10: its audited
        // summary closes the domain empty, and `semantic-model.md` § reads
        // [Decision 2026-09-10] scopes the domain to a proxy the export owns,
        // so a caller's props access is not a counter-example to the closure.
        assert!(primitive_performs_no_operation(
            &signals,
            "createTrackedEffect",
            CallClaimDomain::Reads
        ));
        // A domain no dialect admits yet.
        for domain in [
            CallClaimDomain::Callbacks,
            CallClaimDomain::Writes,
            CallClaimDomain::Invalidates,
            CallClaimDomain::Returns,
            CallClaimDomain::Cleanups,
            CallClaimDomain::Disposals,
        ] {
            assert!(!primitive_performs_no_operation(
                &signals,
                "createTrackedEffect",
                domain
            ));
        }
        // A scheduling primitive answers `reads` too: § reads
        // [Decision 2026-09-10] attributes the reads of computations its drain
        // runs to whoever registered them.
        for export in ["action", "flush"] {
            assert!(primitive_performs_no_operation(
                &signals,
                export,
                CallClaimDomain::Reads
            ));
        }
        // Empty and unaudited inputs.
        assert!(!primitive_performs_no_operation(
            &unaudited_archive(""),
            "createTrackedEffect",
            CallClaimDomain::Creates
        ));
        assert!(!primitive_performs_no_operation(
            &signals,
            "",
            CallClaimDomain::Creates
        ));
        assert!(!primitive_performs_no_operation(
            &unaudited_archive("@solidjs/router"),
            "createMemo",
            CallClaimDomain::Creates
        ));
    }

    /// A row answers for the archive it was read on, and a same-named archive
    /// at another version answers only from rows of its own.
    ///
    /// `createOptimisticStore` `reads` is the case that proves the scoping is
    /// real rather than cosmetic: rc.3 carries the row, and the 2026-09-25
    /// re-audit withheld it on rc.6 (its new landing path reads through the
    /// store proxy the call created), so the two archives must disagree.
    #[test]
    fn negative_rows_answer_only_for_the_archive_they_were_read_on() {
        let rc3 = audited_archive("@solidjs/signals", "2.0.0-rc.3");
        let rc6 = audited_archive("@solidjs/signals", "2.0.0-rc.6");

        // The two `creates` rows the ecosystem's census demanded.
        for export in ["getOwner", "onCleanup"] {
            for archive in [&rc3, &rc6] {
                assert!(
                    primitive_performs_no_operation(archive, export, CallClaimDomain::Creates),
                    "{}@{} {export}",
                    archive.name,
                    archive.version
                );
            }
        }

        // Denied on rc.3, withheld on rc.6: the rc.3 row does not travel.
        assert!(primitive_performs_no_operation(
            &rc3,
            "createOptimisticStore",
            CallClaimDomain::Reads
        ));
        assert!(!primitive_performs_no_operation(
            &rc6,
            "createOptimisticStore",
            CallClaimDomain::Reads
        ));
        // Its `creates` row was re-granted on rc.6's own bytes.
        assert!(primitive_performs_no_operation(
            &rc6,
            "createOptimisticStore",
            CallClaimDomain::Creates
        ));

        // A tuple spliced from both archives is neither, and one carrying
        // rc.6's coordinate over other bytes is not rc.6.
        for (why, archive) in [
            (
                "rc.6's coordinate with rc.3's integrity and manifest",
                AuditedArchive {
                    version: rc6.version,
                    ..rc3
                },
            ),
            (
                "rc.6 with another integrity",
                AuditedArchive {
                    integrity: "sha512-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==",
                    ..rc6
                },
            ),
            (
                "rc.6 with another manifest digest",
                AuditedArchive {
                    manifest_sha256: rc3.manifest_sha256,
                    ..rc6
                },
            ),
        ] {
            assert!(
                !primitive_performs_no_operation(&archive, "getOwner", CallClaimDomain::Creates),
                "{why} must deny nothing"
            );
        }
    }

    /// `@solidjs/signals@2.0.0-rc.9` answers from exactly the rows read on its
    /// own bytes: the five `creates` rows of 2026-09-26 and the nineteen of
    /// the 2026-09-27 parity reading, which is rc.6's set, pair for pair.
    ///
    /// `createOptimisticStore` `reads` is the control that matters: rc.3
    /// grants it, rc.6 withholds it (the export's own landing router reads
    /// through the proxy it created), and rc.9 keeps that router, so rc.9 must
    /// stay silent while rc.3 still answers. A domain no audit read on any
    /// archive (`createRoot` `reads`) stays silent too.
    #[test]
    fn rc9_signals_answers_exactly_what_rc6_answers() {
        let rc3 = audited_archive("@solidjs/signals", "2.0.0-rc.3");
        let rc6 = audited_archive("@solidjs/signals", "2.0.0-rc.6");
        let rc9 = audited_archive("@solidjs/signals", "2.0.0-rc.9");
        let granted = [
            ("action", CallClaimDomain::Creates),
            ("action", CallClaimDomain::Reads),
            ("createMemo", CallClaimDomain::Creates),
            ("createMemo", CallClaimDomain::Reads),
            ("createOptimistic", CallClaimDomain::Creates),
            ("createOptimistic", CallClaimDomain::Reads),
            ("createOptimisticStore", CallClaimDomain::Creates),
            ("createProjection", CallClaimDomain::Creates),
            ("createRoot", CallClaimDomain::Creates),
            ("createSignal", CallClaimDomain::Creates),
            ("createStore", CallClaimDomain::Creates),
            ("createTrackedEffect", CallClaimDomain::Creates),
            ("createTrackedEffect", CallClaimDomain::Reads),
            ("flush", CallClaimDomain::Creates),
            ("flush", CallClaimDomain::Reads),
            ("getOwner", CallClaimDomain::Creates),
            ("onCleanup", CallClaimDomain::Creates),
            ("onSettled", CallClaimDomain::Creates),
            ("onSettled", CallClaimDomain::Reads),
            ("reconcile", CallClaimDomain::Creates),
            ("reconcile", CallClaimDomain::Reads),
            ("runWithOwner", CallClaimDomain::Creates),
            ("snapshot", CallClaimDomain::Creates),
            ("untrack", CallClaimDomain::Creates),
        ];
        for (export, domain) in granted {
            assert!(
                primitive_performs_no_operation(&rc9, export, domain),
                "rc.9 {export} {domain:?}"
            );
            assert!(
                primitive_performs_no_operation(&rc6, export, domain),
                "rc.6 {export} {domain:?}: rc.9 grants nothing rc.6 does not"
            );
        }

        // The withholding rc.6 introduced holds on rc.9, and does not reach
        // back to rc.3's row.
        assert!(primitive_performs_no_operation(
            &rc3,
            "createOptimisticStore",
            CallClaimDomain::Reads
        ));
        for archive in [&rc6, &rc9] {
            assert!(
                !primitive_performs_no_operation(
                    archive,
                    "createOptimisticStore",
                    CallClaimDomain::Reads
                ),
                "{} createOptimisticStore reads is withheld",
                archive.version
            );
        }
        for archive in [&rc3, &rc6, &rc9] {
            assert!(
                !primitive_performs_no_operation(archive, "createRoot", CallClaimDomain::Reads),
                "{} createRoot reads was never read",
                archive.version
            );
        }

        // rc.9's coordinate over rc.6's bytes, or with another integrity, is
        // not rc.9.
        for (why, archive) in [
            (
                "rc.9's coordinate with rc.6's integrity and manifest",
                AuditedArchive {
                    version: rc9.version,
                    ..rc6
                },
            ),
            (
                "rc.9 with rc.6's integrity",
                AuditedArchive {
                    integrity: rc6.integrity,
                    ..rc9
                },
            ),
            (
                "rc.9 with rc.6's manifest digest",
                AuditedArchive {
                    manifest_sha256: rc6.manifest_sha256,
                    ..rc9
                },
            ),
        ] {
            for (export, domain) in granted {
                assert!(
                    !primitive_performs_no_operation(&archive, export, domain),
                    "{why} must deny nothing ({export} {domain:?})"
                );
            }
        }
    }

    /// The export has to be a canonical dialect spelling, not merely a key the
    /// audited document happens to close.
    ///
    /// `isEqual` is the case that matters: `@solidjs/signals` really does
    /// export it and its audited summary really does close `creates: []`, but
    /// no dialect models it, so the census has no primitive identity to
    /// terminate on and the table must stay silent.
    #[test]
    fn a_denial_needs_a_canonical_primitive_spelling() {
        assert!(canonical_primitive_name("createTrackedEffect"));
        assert!(canonical_primitive_name("createEffect"));
        assert!(!canonical_primitive_name("isEqual"));
        assert!(!canonical_primitive_name("applyRef"));
        assert!(!canonical_primitive_name(""));
        // 1.x's `effect` is an alias, not the canonical spelling of
        // `createRenderEffect`, so it is not a canonical name.
        assert!(!canonical_primitive_name("effect"));
        assert!(!primitive_performs_no_operation(
            &audited_archive("@solidjs/signals", "2.0.0-rc.3"),
            "isEqual",
            CallClaimDomain::Creates
        ));
    }

    /// The identity tuples a caller must bind before consulting the table.
    #[test]
    fn audited_archives_are_looked_up_by_name_and_carry_all_four_fields() {
        // Three `@solidjs/signals` archives, each read on its own bytes: rc.3,
        // rc.6 (the one the ecosystem installs, 2026-09-25 re-audit), and
        // rc.9 (solid-primitives' `next`, five rows, 2026-09-26).
        let signals = audited_archives("@solidjs/signals");
        assert_eq!(
            signals
                .iter()
                .map(|archive| archive.version)
                .collect::<Vec<_>>(),
            ["2.0.0-rc.3", "2.0.0-rc.6", "2.0.0-rc.9", "2.0.0-rc.13"]
        );
        for archive in &signals {
            assert!(archive.integrity.starts_with("sha512-"));
            assert_eq!(archive.manifest_sha256.len(), 64);
        }
        for (index, left) in signals.iter().enumerate() {
            for right in &signals[index + 1..] {
                assert_ne!(left.integrity, right.integrity);
                assert_ne!(left.manifest_sha256, right.manifest_sha256);
            }
        }
        assert!(audited_archives("").is_empty());
        assert!(audited_archives("@solidjs/router").is_empty());
        // Two archives named `solid-js` are audited -- rc.3 and, since
        // 2026-09-27, rc.9 -- now that the 1.x dialect, which audited
        // `solid-js@1.9.14`, is gone; `@solidjs/web` the same two.
        for name in ["solid-js", "@solidjs/web"] {
            assert_eq!(
                audited_archives(name)
                    .iter()
                    .map(|archive| archive.version)
                    .collect::<Vec<_>>(),
                ["2.0.0-rc.3", "2.0.0-rc.9", "2.0.0-rc.13"],
                "{name}"
            );
        }
    }

    /// `solid-js@2.0.0-rc.9` and `@solidjs/web@2.0.0-rc.9` answer from exactly
    /// the rows read on their own bytes (2026-09-27), and from nothing rc.3
    /// carries beyond them.
    ///
    /// The controls are rows rc.3 still grants and rc.9's reading withheld:
    /// the four `solid-js` names rc.9 re-exports from `@solidjs/signals`
    /// (`affects`, `isPending`, `latest`, `refresh`). Every other rc.3 row of
    /// these two packages was either re-granted on rc.9 or withdrawn on rc.3
    /// too.
    #[test]
    fn rc9_core_and_web_answer_only_their_own_rows() {
        let js3 = audited_archive("solid-js", "2.0.0-rc.3");
        let js9 = audited_archive("solid-js", "2.0.0-rc.9");
        let web3 = audited_archive("@solidjs/web", "2.0.0-rc.3");
        let web9 = audited_archive("@solidjs/web", "2.0.0-rc.9");
        for (archive, export, domain) in [
            (js9, "For", CallClaimDomain::Reads),
            (js9, "For", CallClaimDomain::Creates),
            (js9, "Repeat", CallClaimDomain::Reads),
            (js9, "Repeat", CallClaimDomain::Creates),
            (js9, "Match", CallClaimDomain::Reads),
            (js9, "Match", CallClaimDomain::Creates),
            (js9, "createContext", CallClaimDomain::Creates),
            (js9, "useContext", CallClaimDomain::Creates),
            (web9, "clientOnly", CallClaimDomain::Reads),
            (web9, "clientOnly", CallClaimDomain::Creates),
            (web9, "httpHeader", CallClaimDomain::Reads),
            (web9, "httpHeader", CallClaimDomain::Creates),
            (web9, "httpStatus", CallClaimDomain::Reads),
            (web9, "httpStatus", CallClaimDomain::Creates),
        ] {
            assert!(
                primitive_performs_no_operation(&archive, export, domain),
                "{}@{} {export} {domain:?}",
                archive.name,
                archive.version
            );
        }
        // Withheld on rc.9, for a reach of the export's own closure.
        for (archive, export, domain) in [
            (js9, "Show", CallClaimDomain::Reads),
            (js9, "Show", CallClaimDomain::Creates),
            (js9, "Loading", CallClaimDomain::Creates),
            (js9, "createSignal", CallClaimDomain::Creates),
            (web9, "hydrate", CallClaimDomain::Reads),
            (web9, "render", CallClaimDomain::Reads),
            (web9, "hydrate", CallClaimDomain::Creates),
            (web9, "render", CallClaimDomain::Creates),
        ] {
            assert!(
                !primitive_performs_no_operation(&archive, export, domain),
                "{}@{} {export} {domain:?} is withheld",
                archive.name,
                archive.version
            );
        }
        // The scoped `createSignal` row is not a denial.
        assert!(host_target_row(&js9, "createSignal", CallClaimDomain::Creates).is_some());
        // 2026-09-28: `createMemo` is `solid-js`' own declaration, withheld
        // flat for its server body and scoped to `browser`; `omit` is
        // `@solidjs/signals`' and denied everywhere, and `merge` is withheld
        // for `solid-js`' server `merge`, which no row can narrow yet.
        let signals9 = audited_archive("@solidjs/signals", "2.0.0-rc.9");
        assert!(!primitive_performs_no_operation(
            &js9,
            "createMemo",
            CallClaimDomain::Creates
        ));
        let memo = host_target_row(&js9, "createMemo", CallClaimDomain::Creates)
            .expect("the browser-scoped createMemo row");
        assert_eq!(memo.condition, HostTargetCondition::Browser);
        assert!(primitive_performs_no_operation(
            &signals9,
            "omit",
            CallClaimDomain::Creates
        ));
        assert!(!primitive_performs_no_operation(
            &signals9,
            "merge",
            CallClaimDomain::Creates
        ));
        assert!(host_target_row(&signals9, "merge", CallClaimDomain::Creates).is_none());
        assert!(!primitive_performs_no_operation(
            &signals9,
            "omit",
            CallClaimDomain::Reads
        ));
        // rc.3 rows do not stand in for rc.9: the re-exported names.
        for (export, domain) in [
            ("affects", CallClaimDomain::Reads),
            ("affects", CallClaimDomain::Creates),
            ("isPending", CallClaimDomain::Creates),
            ("latest", CallClaimDomain::Creates),
            ("refresh", CallClaimDomain::Reads),
            ("refresh", CallClaimDomain::Creates),
        ] {
            assert!(primitive_performs_no_operation(&js3, export, domain));
            assert!(
                !primitive_performs_no_operation(&js9, export, domain),
                "rc.3's {export} {domain:?} row must not answer for rc.9"
            );
        }
        assert!(primitive_performs_no_operation(
            &web3,
            "clientOnly",
            CallClaimDomain::Creates
        ));

        // rc.9's coordinate over rc.3's bytes, or with another integrity, is
        // not rc.9.
        for (why, archive) in [
            (
                "solid-js rc.9's coordinate with rc.3's integrity and manifest",
                AuditedArchive {
                    version: js9.version,
                    ..js3
                },
            ),
            (
                "solid-js rc.9 with rc.3's integrity",
                AuditedArchive {
                    integrity: js3.integrity,
                    ..js9
                },
            ),
            (
                "@solidjs/web rc.9 with rc.3's manifest digest",
                AuditedArchive {
                    manifest_sha256: web3.manifest_sha256,
                    ..web9
                },
            ),
        ] {
            for export in ["For", "createContext", "clientOnly"] {
                assert!(
                    !primitive_performs_no_operation(&archive, export, CallClaimDomain::Creates),
                    "{why} must deny nothing ({export})"
                );
            }
        }
    }

    /// The domain vocabulary is the eight kinded call claim domains, and
    /// `throws` is deliberately not one of them.
    #[test]
    fn call_claim_domains_name_their_wire_key_and_their_one_operation_kind() {
        for (domain, wire, kind) in [
            (CallClaimDomain::Callbacks, "callbacks", "invoke"),
            (CallClaimDomain::Reads, "reads", "read"),
            (CallClaimDomain::Writes, "writes", "write"),
            (CallClaimDomain::Creates, "creates", "create"),
            (CallClaimDomain::Invalidates, "invalidates", "invalidate"),
            (CallClaimDomain::Returns, "returns", "return"),
            (CallClaimDomain::Cleanups, "cleanups", "cleanup"),
            (CallClaimDomain::Disposals, "disposals", "dispose"),
        ] {
            assert_eq!(domain.wire_name(), wire);
            assert_eq!(domain.operation_kind(), kind);
        }
    }

    /// `CallClaimDomain`'s wire name and operation kind are the normalized
    /// contract's own vocabulary, not a private spelling this crate could
    /// drift from silently. Pinned directly against the published schema
    /// rather than against a copy of its enum.
    #[test]
    fn call_claim_domain_vocabulary_matches_the_published_schema() {
        let schema_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("schema/solid-reactivity.schema.json");
        let schema: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&schema_path).unwrap_or_else(|error| {
                panic!("{} is not readable: {error}", schema_path.display())
            }))
            .expect("valid schema JSON");
        let call_domain_enum = schema["$defs"]["callDomain"]["enum"]
            .as_array()
            .expect("callDomain is an enum")
            .iter()
            .map(|value| value.as_str().expect("a string"))
            .collect::<Vec<_>>();
        let operation_kind_enum = schema["$defs"]["operation"]["properties"]["kind"]["enum"]
            .as_array()
            .expect("operation.kind is an enum")
            .iter()
            .map(|value| value.as_str().expect("a string"))
            .collect::<Vec<_>>();

        for domain in [
            CallClaimDomain::Callbacks,
            CallClaimDomain::Reads,
            CallClaimDomain::Writes,
            CallClaimDomain::Creates,
            CallClaimDomain::Invalidates,
            CallClaimDomain::Returns,
            CallClaimDomain::Cleanups,
            CallClaimDomain::Disposals,
        ] {
            assert!(
                call_domain_enum.contains(&domain.wire_name()),
                "{} is not in the schema's callDomain enum",
                domain.wire_name()
            );
            assert!(
                operation_kind_enum.contains(&domain.operation_kind()),
                "{} is not in the schema's operation.kind enum",
                domain.operation_kind()
            );
        }
        // `throws` is the one domain name the schema carries that
        // `CallClaimDomain` deliberately omits — it constrains no operation
        // kind (`semantic-model.md` § "What a closed call domain denies").
        assert_eq!(call_domain_enum.len(), 9);
        assert!(call_domain_enum.contains(&"throws"));
    }

    /// `audited-archives.json` mirrors what each authority audited.
    ///
    /// The identity gate that admits a negative row
    /// (`contract_certification/type_facts.rs`) binds name, version, integrity
    /// and manifest digest field by field, so an authority answers only about
    /// the exact bytes it was read against. Which installed trees those are is
    /// a question consumers outside this crate need — the ecosystem
    /// benchmark's coverage gate asks it of every probe row — and nothing
    /// published it. The file answers it, and this test is what keeps it from
    /// drifting: a re-audit that changes a tuple, adds an archive, or moves a
    /// row count fails here until the file follows.
    ///
    /// Matching a tuple in that file is never a proof. It says the identity
    /// gate *could* be reached, and nothing about whether any row denies
    /// anything — `primitive_performs_no_operation` still decides that, and
    /// silence there is never "no".
    #[test]
    fn audited_archives_json_mirrors_the_dialect_tables() {
        let document: serde_json::Value =
            serde_json::from_str(include_str!("../audited-archives.json"))
                .expect("audited-archives.json is valid JSON");
        assert_eq!(document["schemaVersion"], 1);
        let published = document["dialects"]
            .as_array()
            .expect("audited-archives.json carries a dialects array");
        // Spelled the way the dialect assembly manifests spell it
        // (`rust/dialects/<id>/dialect.json`), because that is the identifier
        // every consumer outside Rust already keys on.
        let expected = [(&Solid2 as &dyn Dialect, "solid-v2")];
        assert_eq!(
            published.len(),
            expected.len(),
            "every dialect must publish its authority, empty or not: an absent \
             entry and an empty one are the difference between \"never looked\" \
             and \"looked and audited nothing\""
        );
        for ((dialect, id), entry) in expected.into_iter().zip(published) {
            assert_eq!(entry["id"], id);
            let authority = dialect.negative_claim_authority();
            assert_eq!(
                entry["negativeRowCount"].as_u64(),
                Some(authority.rows.len() as u64),
                "{id} negative row count"
            );
            let archives = entry["archives"]
                .as_array()
                .unwrap_or_else(|| panic!("{id} carries an archives array"));
            assert_eq!(
                archives.len(),
                authority.archives.len(),
                "{id} audited archive count"
            );
            for (audited, archive) in authority.archives.iter().zip(archives) {
                assert_eq!(archive["name"], audited.name, "{id} archive name");
                assert_eq!(
                    archive["version"], audited.version,
                    "{id} {} version",
                    audited.name
                );
                assert_eq!(
                    archive["integrity"], audited.integrity,
                    "{id} {} integrity",
                    audited.name
                );
                assert_eq!(
                    archive["manifestSha256"], audited.manifest_sha256,
                    "{id} {} manifest digest",
                    audited.name
                );
            }
        }
    }
}
