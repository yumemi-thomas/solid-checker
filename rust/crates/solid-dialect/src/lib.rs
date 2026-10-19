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
    ContractTrigger,
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
    /// and stays absent until this dialect's owner has one, which is why the
    /// aggregate [`unambiguous_reactive_result_slot`] treats one dialect's
    /// silence as disagreement rather than deferring to the other's row.
    fn reactive_result_slot(&self, primitive: Primitive, slot: ResultSlot) -> Option<ReactiveRole> {
        let _ = (primitive, slot);
        None
    }

    /// Exact declaration identities whose runtime use withholds the bounded
    /// captured-lookup proof (mode switches, node exposure, diagnostic footer).
    /// This is a recipe refusal, not another runtime-configuration premise.
    fn captured_lookup_runtime_hazard(&self, declaration_path: &str, start: u64, end: u64) -> bool {
        let _ = (declaration_path, start, end);
        false
    }

    /// Under the explicit standard runtime configuration and intact runtime
    /// model, a noncallable initial value, no options and a discarded setter leave
    /// this tuple accessor observing precisely the initial value. The consumer
    /// must also prove no accessor/result escape can expose the source or value.
    /// Silence is not preservation. This does not generalize to derived sources.
    fn tuple_accessor_preserves_initial_value(
        &self,
        primitive: Primitive,
        slot: ResultSlot,
    ) -> bool {
        let _ = (primitive, slot);
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
mod tests {
    use super::*;

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
    fn tuple_initial_value_preservation_requires_a_dialect_row() {
        let silent = &Silent as &dyn Dialect;
        assert!(!silent.tuple_accessor_preserves_initial_value(
            Primitive::CreateSignal,
            ResultSlot::TupleItem(0)
        ));
        let solid = &Solid2 as &dyn Dialect;
        let path = "/p/node_modules/@solidjs/signals/dist/types/core/verdict.d.ts";
        assert!(solid.captured_lookup_runtime_hazard(path, 75, 84));
        assert!(!silent.captured_lookup_runtime_hazard(path, 75, 84));
        assert!(!solid.captured_lookup_runtime_hazard(path, 74, 84));
        assert!(!solid.captured_lookup_runtime_hazard(
            "/p/node_modules/other/verdict.d.ts",
            75,
            84
        ));
        assert!(solid.tuple_accessor_preserves_initial_value(
            Primitive::CreateSignal,
            ResultSlot::TupleItem(0)
        ));
        assert!(!solid.tuple_accessor_preserves_initial_value(
            Primitive::CreateSignal,
            ResultSlot::TupleItem(1)
        ));
        assert!(
            !solid.tuple_accessor_preserves_initial_value(Primitive::CreateMemo, ResultSlot::Whole)
        );
        assert!(!solid.tuple_accessor_preserves_initial_value(
            Primitive::CreateOptimistic,
            ResultSlot::TupleItem(0)
        ));
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

    #[test]
    fn callback_result_identity_is_an_explicit_dialect_claim() {
        let two = &Solid2 as &dyn Dialect;
        assert_eq!(two.returned_callback_value(Primitive::CreateRoot), Some(0));
        for primitive in [
            Primitive::Untrack,
            Primitive::RunWithOwner,
            Primitive::CreateMemo,
        ] {
            assert_eq!(two.returned_callback_value(primitive), None);
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
}
