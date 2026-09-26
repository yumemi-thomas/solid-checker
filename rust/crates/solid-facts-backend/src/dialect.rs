//! The composition seam between dialect-independent infrastructure and one
//! Solid dialect.
//!
//! A [`Dialect`] bundles everything a Solid version contributes to the
//! checker: its vocabulary, its compiler adapter, its rule catalog, its rule
//! documentation, and its built-in runtime model, plus the stable identity
//! that keys every cache and retained session. The analysis pipeline receives
//! the selected `&Dialect` from its entry point — the CLI's `--dialect` flag,
//! the wasm request, or [`detect`] when a request names none — and never
//! names a dialect crate directly.

use std::path::{Path, PathBuf};

use solid_facts::compiler::CompilerFactsProvider;
use solid_reactive_ir::{Finding, Program, RuleMetadata, SolveTimings};

/// Rule identities this checker used to publish and has since removed, with the
/// reason, so a project's existing `.solid-checker/rule-options.json` does not
/// hard-fail on an id that no longer exists.
///
/// A rule name in that document is validated against the catalogs, and an
/// unknown name fails the whole analysis rather than silently changing policy —
/// which is right for a typo and wrong for a rule the checker itself deleted.
/// Accepting a retired id is **not** demoting the rule or hiding it behind an
/// option (AGENTS.md forbids both): the rule cannot fire, no catalog declares
/// it, and disabling it is a no-op. Only the stale key is tolerated.
///
/// Entries are permanent. Removing one turns a tolerated config back into a
/// fatal error for the same user, so this list only grows.
/// The note for every `v1/` identity the 1.x catalog still declared when it was
/// deleted. They were not retired one at a time for reasons of their own, so
/// they share one sentence rather than repeating eighteen variants of it.
///
/// The `v1/` rows listed individually in [`RETIRED_RULES`] keep their own
/// notes: a reader whose `v1/imports` disable stopped working needs "this
/// claim was TypeScript's", and "the dialect is gone" would not tell them that.
const RETIRED_WITH_THE_V1_CATALOG: &str = "removed 2026-09-16: the Solid 1.x catalog was deleted with its dialect (ADR 0110); a project whose installed solid-js resolves to 1.x is refused with SC9013 rather than analyzed under another catalog";

pub const RETIRED_RULES: &[(&str, &str)] = &[
    (
        "invalid-cleanup-return",
        "removed 2026-08-17: every illegal return is a TypeScript error against `EffectFunction`'s `(() => void) | void` return type",
    ),
    (
        "cleanup-return-unresolved",
        "removed 2026-08-17: the obligation's whole domain was the legality of a returned value, which the same type closes",
    ),
    (
        "invalid-refresh-target",
        "removed 2026-08-17: `Refreshable<T>` brands the target in the type system, so every invalid target is a TypeScript error",
    ),
    (
        "invalid-affects-target",
        "removed 2026-08-17: same, against `Accessor<unknown> | Store<object>`",
    ),
    (
        "affects-keys-on-accessor",
        "removed 2026-08-17: a key on an accessor target selects the one-argument overload, so the key is a TypeScript error",
    ),
    (
        "refresh-target-unresolved",
        "removed 2026-08-17: asked whether the target carries the source brand, which is a question the type answers",
    ),
    ("affects-target-unresolved", "removed 2026-08-17: same"),
    (
        "v1/imports",
        "removed 2026-08-17: its one condition — the named module does not export the name — is exactly TS2305's",
    ),
    (
        "v1/untracked-derived-function",
        "removed 2026-08-20: SC1001 follows helper-call chains and owns the same untracked reactive-read failure",
    ),
    (
        "untracked-derived-function",
        "removed 2026-08-20: SC1001 follows helper-call chains and owns the same runtime STRICT_READ_UNTRACKED failure",
    ),
    (
        "v1/cleanup-in-forbidden-scope",
        "removed 2026-08-20: Solid 1.x createReaction callbacks run under the reaction's own disposing computation",
    ),
    (
        "v1/primitive-in-leaf-owner",
        "removed 2026-08-20: Solid 1.x createReaction owns and disposes primitives created by its invalidation callback",
    ),
    (
        "v1/primitive-in-directive-application",
        "removed 2026-08-20: Solid 1.x directive and ref application preserve the surrounding owner",
    ),
    (
        "v1/no-implicit-draggable",
        "removed 2026-08-20: its inverted shorthand check was generic HTML attribute-state validation, outside the checker domain",
    ),
    (
        "no-implicit-draggable",
        "removed 2026-08-20: the remaining claim was generic HTML draggable-state validation, outside the checker domain",
    ),
    (
        "v1/no-array-handlers",
        "removed 2026-08-20: Solid 1.x intentionally supports [handler, data] pairs, and the available facts cannot prove that a matched pair is defective",
    ),
    (
        "v1/no-react-deps",
        "removed 2026-08-20: Solid 1.x intentionally accepts an array seed and passes it to the reactive callback",
    ),
    (
        "v1/event-handlers",
        "removed 2026-08-20: its surviving arms enforced spelling and intent conventions rather than proven runtime defects",
    ),
    (
        "v1/no-react-specific-props",
        "removed 2026-08-20: intrinsic uses are TypeScript errors and component props are passed through without React-specific lowering",
    ),
    (
        "v1/no-unknown-namespaces",
        "removed 2026-08-20: namespaced component props are delivered verbatim and intrinsic invalid names are TypeScript-owned",
    ),
    (
        "v1/no-innerhtml",
        "removed 2026-08-20: its component and injection-policy arms were unproven; content competition remains SC8003",
    ),
    (
        "v1/style-prop",
        "removed 2026-08-20: its component arm was false and its intrinsic residue was CSS style policy or TypeScript-owned",
    ),
    (
        "v1/no-async-tracked-scope",
        "removed 2026-08-20: an async tracked callback is not itself defective; SC1002 reports only proven reactive reads after await",
    ),
    (
        "v1/jsx-no-script-url",
        "removed 2026-08-20: generic injection-sink policy is outside the checker domain",
    ),
    (
        "v1/jsx-uses-vars",
        "removed 2026-08-20: it never emitted a diagnostic because semantic reference facts already model JSX uses",
    ),
    (
        "v1/no-proxy-apis",
        "removed 2026-08-20: runtime target compatibility is project policy and cannot be proven from source",
    ),
    (
        "v1/self-closing-comp",
        "removed 2026-08-20: self-closing syntax is formatting, not a runtime defect",
    ),
    (
        "v1/prefer-component-syntax",
        "removed 2026-08-20: imperative calls of JSX-returning functions are runtime-valid and the rule enforced a naming convention",
    ),
    (
        "prefer-component-syntax",
        "removed 2026-08-20: imperative calls of JSX-returning functions are runtime-valid and the rule enforced a naming convention",
    ),
    (
        "v1/execution-map-incomplete",
        "removed 2026-08-20: compiler-fact completeness is a producer-integrity invariant, not a project diagnostic",
    ),
    (
        "execution-map-incomplete",
        "removed 2026-08-20: compiler-fact completeness is a producer-integrity invariant, not a project diagnostic",
    ),
    (
        "v1/valid-jsx-nesting",
        "removed 2026-08-20: generic HTML parser conformance is outside the Solid semantic checker domain",
    ),
    (
        "valid-jsx-nesting",
        "removed 2026-08-20: generic HTML parser conformance is outside the Solid semantic checker domain",
    ),
    (
        "cleanup-in-forbidden-scope",
        "merged 2026-08-20 into leaf-owner-forbidden-call; existing disables intentionally do not transfer to the wider family",
    ),
    (
        "primitive-in-leaf-owner",
        "merged 2026-08-20 into leaf-owner-forbidden-call; existing disables intentionally do not transfer to the wider family",
    ),
    (
        "flush-in-forbidden-scope",
        "merged 2026-08-20 into leaf-owner-forbidden-call; existing disables intentionally do not transfer to the wider family",
    ),
    (
        "pending-async-untracked-read",
        "merged 2026-08-20 into pending-async-unsuspendable-read; existing disables intentionally do not transfer to the wider family",
    ),
    (
        "pending-async-forbidden-scope",
        "merged 2026-08-20 into pending-async-unsuspendable-read; existing disables intentionally do not transfer to the wider family",
    ),
    (
        "ssr-client-source-outside-loading-boundary",
        "merged 2026-08-20 into async-outside-loading-boundary; existing disables intentionally do not transfer to the wider rule",
    ),
    // The 18 the 1.x catalog still declared at deletion. Listed by name rather
    // than matched by a `v1/` prefix so a typo inside the retired namespace is
    // still refused -- the whole point of this document's validation is that a
    // misspelling must not silently mean "defaults".
    ("v1/strict-read-untracked", RETIRED_WITH_THE_V1_CATALOG),
    ("v1/reactive-read-after-await", RETIRED_WITH_THE_V1_CATALOG),
    ("v1/no-destructure", RETIRED_WITH_THE_V1_CATALOG),
    ("v1/components-return-once", RETIRED_WITH_THE_V1_CATALOG),
    (
        "v1/reactive-write-in-owned-scope",
        RETIRED_WITH_THE_V1_CATALOG,
    ),
    ("v1/missing-owner", RETIRED_WITH_THE_V1_CATALOG),
    ("v1/missing-effect-function", RETIRED_WITH_THE_V1_CATALOG),
    ("v1/uncalled-accessor", RETIRED_WITH_THE_V1_CATALOG),
    ("v1/reactive-handler-frozen", RETIRED_WITH_THE_V1_CATALOG),
    ("v1/no-direct-mutation", RETIRED_WITH_THE_V1_CATALOG),
    ("v1/reactive-source-uncaptured", RETIRED_WITH_THE_V1_CATALOG),
    (
        "v1/reactive-dispatch-unresolved",
        RETIRED_WITH_THE_V1_CATALOG,
    ),
    ("v1/jsx-no-duplicate-props", RETIRED_WITH_THE_V1_CATALOG),
    ("v1/jsx-no-undef", RETIRED_WITH_THE_V1_CATALOG),
    ("v1/prefer-classlist", RETIRED_WITH_THE_V1_CATALOG),
    ("v1/prefer-for", RETIRED_WITH_THE_V1_CATALOG),
    ("v1/prefer-show", RETIRED_WITH_THE_V1_CATALOG),
    (
        "v1/package-contract-incomplete",
        RETIRED_WITH_THE_V1_CATALOG,
    ),
];

/// Former external rule identities that canonicalize onto a current rule.
///
/// Unlike [`RETIRED_RULES`], an alias transfers configuration: disabling its
/// old name disables the current target. Each entry landed atomically with
/// the identity change that created its target.
pub const RULE_ALIASES: &[(&str, &str)] = &[
    ("v1/no-owner-effect", "v1/missing-owner"),
    ("v1/no-owner-cleanup", "v1/missing-owner"),
    ("v1/no-owner-boundary", "v1/missing-owner"),
    ("no-owner-effect", "missing-owner"),
    ("no-owner-cleanup", "missing-owner"),
    ("no-owner-boundary", "missing-owner"),
    ("no-owner-settled-cleanup", "missing-owner"),
    (
        "v1/package-contract-export-missing",
        "v1/package-contract-incomplete",
    ),
    (
        "v1/package-contract-missing",
        "v1/package-contract-incomplete",
    ),
    (
        "v1/package-contract-callback-missing",
        "v1/package-contract-incomplete",
    ),
    (
        "package-contract-export-missing",
        "package-contract-incomplete",
    ),
    ("package-contract-missing", "package-contract-incomplete"),
    (
        "package-contract-callback-missing",
        "package-contract-incomplete",
    ),
    ("component-props-destructure", "no-destructure"),
    ("component-returns-conditionally", "components-return-once"),
    (
        "expected-function-got-expression",
        "reactive-handler-frozen",
    ),
    (
        "v1/expected-function-got-expression",
        "v1/reactive-handler-frozen",
    ),
    ("resolve-in-reactive-scope", "resolve-in-tracked-scope"),
    (
        "sync-node-received-async",
        "sync-computation-received-async",
    ),
];

/// The removal note for a retired rule identity, or `None` if the checker never
/// published that name.
#[must_use]
pub fn retired_rule(name: &str) -> Option<&'static str> {
    RETIRED_RULES
        .iter()
        .find(|(retired, _)| *retired == name)
        .map(|(_, reason)| *reason)
}

/// The current catalog identity for a former external name.
#[must_use]
pub fn rule_alias(name: &str) -> Option<&'static str> {
    RULE_ALIASES
        .iter()
        .find(|(old, _)| *old == name)
        .map(|(_, current)| *current)
}

/// Semantic evidence a dialect's catalog needs Type Facts to acquire.
///
/// These are analysis capabilities, not external rule identities. Renaming a
/// rule therefore cannot silently change the fact plan.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SemanticDemandCapabilities {
    pub array_map_receiver_types: bool,
    pub async_array_map_callbacks: bool,
    pub server_argument_library_types: bool,
}

impl SemanticDemandCapabilities {
    pub const NONE: Self = Self {
        array_map_receiver_types: false,
        async_array_map_callbacks: false,
        server_argument_library_types: false,
    };
    /// Only the 2.0 catalog carries `server-function-rich-argument`, so only it
    /// pays for the library-type identities that rule reads.
    const SOLID_2: Self = Self {
        array_map_receiver_types: true,
        async_array_map_callbacks: true,
        server_argument_library_types: true,
    };
}

pub struct Dialect {
    /// Stable identity, folded into every cache key and retained session
    /// identity so artifacts from different dialects can never collide.
    pub id: &'static str,
    /// Exact compiler semantic producer identity. This is separate from the
    /// dialect because a producer pin or trace revision can change answers
    /// while the language dialect remains the same.
    pub compiler_facts_identity: &'static str,
    /// The Solid-version vocabulary the reactive IR analyzes with: which
    /// names are primitives, where their callbacks sit, which JSX tags open
    /// boundaries. The engine asks this table; it never names a version.
    pub vocabulary: &'static dyn solid_dialect::Dialect,
    /// Size of the rule catalog; reporting only.
    pub rule_count: usize,
    /// Constructs the dialect's in-process compiler-facts provider.
    pub compiler: fn() -> Box<dyn CompilerFactsProvider>,
    /// Runs the dialect's rule catalog over a program.
    pub solve_measured: fn(&Program) -> (Vec<Finding>, SolveTimings),
    /// Documentation page for a rule, addressed by its externally visible
    /// name.
    pub docs_url: fn(&str) -> String,
    /// Whether the catalog carries a rule with this externally visible name.
    /// Lets shared backend code condition work on catalog capability
    /// (for example, which type facts to demand) instead of naming a
    /// version.
    pub has_rule: fn(&str) -> bool,
    /// Catalog metadata for one exact external rule identity.
    pub rule_metadata: fn(&str) -> Option<RuleMetadata>,
    /// Typed fact-acquisition requirements of the catalog.
    pub semantic_demands: SemanticDemandCapabilities,
    /// Dialect-owned projection policy used after rule enablement filtering.
    pub catalog_capabilities: solid_reactive_ir::CatalogCapabilities,
}

impl Dialect {
    pub fn solve(&self, program: &Program) -> Vec<Finding> {
        (self.solve_measured)(program).0
    }
}

/// The stable id and nothing else. A dialect is function pointers, a `&dyn`
/// vocabulary table and a rule catalog; the id is the only part of it that
/// means anything in a diagnostic, and it is the part that keys every cache.
impl std::fmt::Debug for Dialect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("Dialect").field(&self.id).finish()
    }
}

/// Every dialect the checker can run with. A new dialect registers here and
/// becomes selectable by id everywhere a dialect can be named.
pub static ALL: &[&Dialect] = &[&SOLID_V2];

// `default_dialect` below has no honest answer for an empty registry, and a
// build that carried no dialect could not analyze anything anyway. Say so at
// compile time rather than at the first request.
const _: () = assert!(!ALL.is_empty(), "a build must carry at least one dialect");

/// Release-specific variants of a registered dialect, each keyed by the exact
/// installed release a vocabulary review names ([`solid_dialect::ReleaseReview`]).
///
/// Not in [`ALL`]: a variant is not a language. It is the same catalog,
/// compiler and runtime model with a vocabulary that answers one question
/// differently for one reviewed release (B1 in the rc.9 review: the store
/// root typing). It has an id of its own because the id keys every cache,
/// retained session and daemon socket, and a result computed under one
/// vocabulary must never answer for the other; [`by_id`] resolves it so the
/// daemon can forward the selection it hashed.
pub static RELEASE_VARIANTS: &[(&str, &Dialect)] = &[("2.0.0-rc.9", &SOLID_V2_RC9)];

/// Resolves a dialect by its stable id: a registered language, or one of its
/// [`RELEASE_VARIANTS`].
#[must_use]
pub fn by_id(id: &str) -> Option<&'static Dialect> {
    ALL.iter()
        .copied()
        .chain(RELEASE_VARIANTS.iter().map(|(_, variant)| *variant))
        .find(|dialect| dialect.id == id)
}

/// The dialect that analyzes one installed release of `language`'s major:
/// the variant its review names, or the language itself.
fn for_release(
    language: &'static Dialect,
    review: solid_dialect::ReleaseReview,
) -> &'static Dialect {
    let solid_dialect::ReleaseReview::ReviewedWithGaps(reviewed) = review else {
        return language;
    };
    RELEASE_VARIANTS
        .iter()
        .find(|(release, variant)| {
            *release == reviewed.version
                && variant.vocabulary.version() == language.vocabulary.version()
        })
        .map_or(language, |(_, variant)| *variant)
}

/// The dialect entry points fall back to when a request names none and
/// nothing resolves: **the newest one [`ALL`] carries**.
///
/// Read from the registry rather than named by a literal. A `Defaulted`
/// detection is an absence — no installed `solid-js`, or a manifest whose
/// version field is not a version — and the language a project that states no
/// version most likely means is the current one. A literal would have gone on
/// answering the *older* default the day a newer dialect was added, and
/// nothing would have said so; `Version`'s ordering is declaration order, so
/// this follows the registry instead.
#[must_use]
pub fn default_dialect() -> &'static Dialect {
    ALL.iter()
        .copied()
        .max_by_key(|dialect| dialect.vocabulary.version())
        .expect("the const assertion above holds ALL non-empty")
}

/// The dialect for a Solid language version, if this build includes it.
#[must_use]
pub fn by_version(version: solid_dialect::Version) -> Option<&'static Dialect> {
    ALL.iter()
        .copied()
        .find(|dialect| dialect.vocabulary.version() == version)
}

/// The diagnostic identity of the unsupported-runtime refusal.
///
/// Held here, not read from a catalog, because the refusal is decided
/// **before** a dialect is chosen: it has to be emittable in any feature
/// configuration, including one whose default catalog does not declare it.
/// The Solid 2 catalog declares it too -- that is where adapters, suppression
/// configuration and `docs/rules/` look it up -- and
/// `the_refusal_identity_is_the_one_the_catalog_publishes` pins the two
/// together so they cannot drift. See ADR 0110.
pub const UNSUPPORTED_RUNTIME_CODE: &str = "SC9013";
/// The rule name paired with [`UNSUPPORTED_RUNTIME_CODE`].
pub const UNSUPPORTED_RUNTIME_RULE: &str = "unsupported-solid-runtime";

/// The diagnostic identity of the unaudited-release notice: the installed
/// release is of a carried major, and the vocabulary was not audited on it.
///
/// Held here for the same reason as [`UNSUPPORTED_RUNTIME_CODE`] -- detection
/// decides it, not a rule -- and pinned to the catalog the same way.
pub const UNAUDITED_RELEASE_CODE: &str = "SC9014";
/// The rule name paired with [`UNAUDITED_RELEASE_CODE`].
pub const UNAUDITED_RELEASE_RULE: &str = "unaudited-solid-release";

/// What the dialect walk found, and where it found it.
///
/// The three cases are **not** the same answer, and a caller that must refuse
/// an unsupported runtime cannot tell them apart from a dialect alone:
///
/// - an installed `solid-js` whose major this build carries,
/// - an installed `solid-js` whose major this build has **no** dialect for —
///   whether or not [`solid_dialect::Version`] even names that major,
/// - nothing installed, or a manifest whose version field is not a version.
///
/// Each case that read a manifest carries the exact path it read, because a
/// refusal has to say *which* `package.json` decided it — the walk is
/// unbounded and the deciding file is frequently not the one beside the
/// project.
#[derive(Clone, Debug)]
pub enum Detection {
    /// The nearest installed `solid-js` names a released major this build
    /// carries a dialect for.
    Installed {
        /// The dialect that analyzes this release: the language, or the
        /// [`RELEASE_VARIANTS`] entry its review names.
        dialect: &'static Dialect,
        version: solid_dialect::Version,
        manifest: PathBuf,
        /// The `version` field exactly as the manifest spelled it.
        installed: String,
        /// The language vocabulary's review of that exact release. Anything
        /// but [`solid_dialect::ReleaseReview::Audited`] means the analysis
        /// carries the `SC9014` notice ([`release_notice`]).
        review: solid_dialect::ReleaseReview,
    },
    /// The nearest installed `solid-js` names a major this build has no
    /// dialect for. **Never a dialect**: there is no correct one to pick, and
    /// picking the default would analyze the project under a language it does
    /// not run. The caller refuses.
    ///
    /// This covers both shapes of "no dialect". `Modelled(V1)` is a major the
    /// build still recognises in order to refuse it; `UnmodelledMajor` is one
    /// no variant names at all, which is the case a future `solid-js@3` lands
    /// in. They refuse identically and on purpose: a dialect that has not been
    /// written yet is not a reason to analyze a project under a different
    /// language, and the alternative — `None` from
    /// [`solid_dialect::Version::for_solid_js`] falling through to
    /// [`Defaulted`](Self::Defaulted) — is precisely the silent-2.0 hole
    /// ADR 0110 § 1 closed for 1.x.
    Unsupported {
        classification: solid_dialect::Classification,
        /// The `version` field exactly as the manifest spelled it. The
        /// refusal quotes this rather than the classified major, because
        /// "1.9.14" tells the reader which install to go and change and
        /// "Solid 1.x" does not.
        installed: String,
        manifest: PathBuf,
        /// `Some` when the major *is* carried and its vocabulary refuses this
        /// release line ([`solid_dialect::ReleaseReview::Refused`]) -- the
        /// pre-beta `2.0.0-experimental.x` today. The refusal then has to say
        /// why, because "carries no dialect for it" would be false.
        refusal: Option<&'static solid_dialect::RefusedRelease>,
    },
    /// Nothing resolved, or the nearest manifest's version field is not a
    /// version (`workspace:*`, an empty or absent field). `manifest` is that
    /// manifest when the walk stopped at one, and `None` when no
    /// `node_modules/solid-js` was found at all.
    ///
    /// A *number* never lands here. `0.5.0` and `3.0.0` are answers about an
    /// installed runtime, and an answer this build cannot honour is refused
    /// rather than defaulted; only an absence defaults.
    Defaulted { manifest: Option<PathBuf> },
}

/// Resolves the dialect a project speaks from the `solid-js` it would
/// actually import: the nearest `node_modules/solid-js/package.json` above
/// the project file, walked the way a bundler resolves.
///
/// Deliberately **not** read from any loaded contract — a bundled contract
/// carries the version the checker ships, not the one the project installed.
///
/// A `detect` that collapsed [`Detection::Unsupported`] onto the default
/// dialect used to sit in front of this. It was the hole ADR 0110 § 1 closed:
/// it analyzed a project under a language it does not run and told it nothing.
/// Every caller reads the [`Detection`] and refuses `Unsupported` with
/// `SC9013`; `Defaulted` — no `node_modules/solid-js`, or a manifest naming no
/// released major — keeps the default, because an absence is not a
/// contradicted answer.
#[must_use]
pub fn detect_detailed(project: &Path) -> Detection {
    let Some((classification, installed, manifest)) = resolved_solid_version(project) else {
        return Detection::Defaulted { manifest: None };
    };
    let Some(classification) = classification else {
        return Detection::Defaulted {
            manifest: Some(manifest),
        };
    };
    if let solid_dialect::Classification::Modelled(version) = classification
        && let Some(language) = by_version(version)
    {
        // The major chose the language; the exact release is the vocabulary's
        // to judge. Asked of the language's own vocabulary, so a variant
        // never reviews the releases that select it.
        let review = language.vocabulary.review_release(&installed);
        if let solid_dialect::ReleaseReview::Refused(refusal) = review {
            return Detection::Unsupported {
                classification,
                installed,
                manifest,
                refusal: Some(refusal),
            };
        }
        return Detection::Installed {
            dialect: for_release(language, review),
            version,
            manifest,
            installed,
            review,
        };
    }
    Detection::Unsupported {
        classification,
        installed,
        manifest,
        refusal: None,
    }
}

/// An installed release the analysis proceeds on without the vocabulary having
/// been audited on it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseNotice {
    /// The `version` field exactly as the manifest spelled it.
    pub installed: String,
    /// The manifest that decided it.
    pub manifest: PathBuf,
    /// [`solid_dialect::ReleaseReview::ReviewedWithGaps`] or
    /// [`solid_dialect::ReleaseReview::Unreviewed`]; never the other two.
    pub review: solid_dialect::ReleaseReview,
}

/// The `SC9014` notice for analyzing `project` under `dialect`, if one is due.
///
/// Due when detection selects exactly this dialect for the installed release
/// and the release is not audited. A dialect detection did *not* select --
/// an explicit `--dialect` naming another vocabulary -- is a decision rather
/// than a detection, and gets no notice, exactly as it gets no `SC9013`.
///
/// Read from disk on every call rather than handed down from the selection
/// site, because the daemon receives only the dialect id it was spawned with
/// and must still say which release it analyzed.
#[must_use]
pub fn release_notice(dialect: &'static Dialect, project: &Path) -> Option<ReleaseNotice> {
    let Detection::Installed {
        dialect: detected,
        manifest,
        installed,
        review,
        ..
    } = detect_detailed(project)
    else {
        return None;
    };
    if detected.id != dialect.id {
        return None;
    }
    match review {
        solid_dialect::ReleaseReview::ReviewedWithGaps(_)
        | solid_dialect::ReleaseReview::Unreviewed => Some(ReleaseNotice {
            installed,
            manifest,
            review,
        }),
        solid_dialect::ReleaseReview::Audited | solid_dialect::ReleaseReview::Refused(_) => None,
    }
}

/// The nearest installed `solid-js`, as
/// `(classification, version as written, manifest path)`.
///
/// The outer `Option` is "did the walk find a manifest carrying a version
/// string at all"; the inner one is whether that string is a version. They are
/// separate answers and the caller needs both: a missing install and an
/// install spelled `workspace:*` both default, but only the second can name
/// the file that decided it. The raw version string rides along because a
/// refusal has to quote what it actually read.
fn resolved_solid_version(
    project: &Path,
) -> Option<(Option<solid_dialect::Classification>, String, PathBuf)> {
    let start = if project.is_dir() {
        project
    } else {
        project.parent()?
    };
    for directory in start.ancestors() {
        let manifest = directory
            .join("node_modules")
            .join("solid-js")
            .join("package.json");
        let Ok(encoded) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        // A manifest that parses to no version string -- broken JSON, or no
        // "version" field -- is treated exactly like an unreadable one: the
        // walk continues, because a broken stub (a half-written install, an
        // empty placeholder) must not mask a real installation higher up.
        let Some(version) = serde_json::from_str::<serde_json::Value>(&encoded)
            .ok()
            .and_then(|manifest| Some(manifest.get("version")?.as_str()?.to_owned()))
        else {
            continue;
        };
        // Whatever the version field says, this is the nearest `solid-js` the
        // project would import, so the walk stops here either way. A string
        // that is not a version at all ("workspace:*") answers `None` and the
        // caller falls back to the default; a major nobody here carries
        // classifies as `UnmodelledMajor` and is refused, because a
        // resolvable-but-uncarried install is a contradicted answer rather
        // than an absence.
        return Some((
            solid_dialect::Version::for_solid_js(&version),
            version,
            manifest,
        ));
    }
    None
}

#[cfg(feature = "dialect-v2")]
static SOLID_V2: Dialect = SOLID_V2_AUDITED;

/// `solid-js@2.0.0-rc.9`: the Solid 2 dialect with the rc.9 vocabulary, whose
/// store root is not `Readonly` (B1). Everything else is the language's.
#[cfg(feature = "dialect-v2")]
static SOLID_V2_RC9: Dialect = Dialect {
    id: "solid-v2@2.0.0-rc.9",
    vocabulary: &solid_dialect::Solid2::RC9,
    ..SOLID_V2_AUDITED
};

#[cfg(feature = "dialect-v2")]
const SOLID_V2_AUDITED: Dialect = Dialect {
    id: "solid-v2",
    compiler_facts_identity: solid_v2_compiler::COMPILER_FACTS_IDENTITY,
    vocabulary: &solid_dialect::Solid2::AUDITED,
    rule_count: solid_v2_rules::Rule::ALL.len(),
    compiler: || Box::new(solid_v2_compiler::NativeCompilerFacts),
    solve_measured: solid_v2_rules::solve_measured,
    docs_url: solid_v2_rules::docs_url,
    has_rule: |name| {
        solid_v2_rules::Rule::ALL
            .into_iter()
            .any(|rule| rule.metadata().name == name)
    },
    rule_metadata: |name| {
        solid_v2_rules::Rule::ALL
            .into_iter()
            .find(|rule| rule.metadata().name == name)
            .map(solid_v2_rules::Rule::metadata)
    },
    semantic_demands: SemanticDemandCapabilities::SOLID_2,
    catalog_capabilities: solid_v2_rules::CATALOG_CAPABILITIES,
};

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use solid_reactive_ir::{
        AsyncRead, DirectMutationTarget, ExecutionRole, OwnerRequirement,
        OwnerRequirementOperation, ReactiveRead, ReactiveWrite, ReactiveWriteOperation,
        StaticDefect, StaticDefectKind,
    };
    use typefacts::Location;

    use super::*;

    /// The package root of a module specifier, matching contract discovery:
    /// `solid-js/store` and `@solidjs/web/frames` are subpaths of one installed
    /// package, not packages of their own.
    fn package_root(module: &str) -> &str {
        if module.starts_with('@') {
            module
                .match_indices('/')
                .nth(1)
                .map_or(module, |(index, _)| &module[..index])
        } else {
            module.split('/').next().unwrap_or(module)
        }
    }

    /// Every package a dialect models is declared in its assembly manifest, and
    /// every declaration models something.
    ///
    /// The manifest drives contract generation, the drift check, runtime
    /// conformance, and the composed-artifact check -- all of which enumerate
    /// what it *declares*. A package the vocabulary owns or the backend bundles
    /// but no entry names is therefore covered by no gate at all: it silently
    /// has no contract, and every project importing it reports SC9005 forever.
    /// This check closes that hole, so it derives the expected set from the
    /// dialect itself rather than from the manifest it is checking.
    #[test]
    fn every_modeled_package_is_declared_in_the_assembly_manifest() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for dialect in ALL {
            let source = root
                .join("rust/dialects")
                .join(dialect.id)
                .join("dialect.json");
            let read = std::fs::read(&source)
                .unwrap_or_else(|error| panic!("{}: {error}", source.display()));
            let manifest: serde_json::Value = serde_json::from_slice(&read)
                .unwrap_or_else(|error| panic!("{}: {error}", source.display()));
            let declared = manifest["contracts"]
                .as_array()
                .unwrap_or_else(|| panic!("{} has no contracts array", source.display()))
                .iter()
                .map(|contract| {
                    contract["package"]
                        .as_str()
                        .unwrap_or_else(|| {
                            panic!("{} has a contract without a package", source.display())
                        })
                        .to_owned()
                })
                .collect::<HashSet<_>>();
            let bundle_index = root.join(
                manifest["bundleIndex"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{} has no bundleIndex", source.display())),
            );
            let bundle: serde_json::Value = serde_json::from_slice(
                &std::fs::read(&bundle_index)
                    .unwrap_or_else(|error| panic!("{}: {error}", bundle_index.display())),
            )
            .unwrap_or_else(|error| panic!("{}: {error}", bundle_index.display()));
            let bundled = bundle["contracts"]
                .as_array()
                .unwrap_or_else(|| panic!("{} has no contracts array", bundle_index.display()))
                .iter()
                .map(|case| {
                    case["package"]
                        .as_str()
                        .unwrap_or_else(|| {
                            panic!("{} has a case without a package", bundle_index.display())
                        })
                        .to_owned()
                })
                .collect::<HashSet<_>>();
            assert!(
                bundled.is_subset(&declared),
                "{} receipt-issued bundle index names an undeclared package",
                dialect.id
            );
            let modeled = dialect
                .vocabulary
                .modules()
                .iter()
                .copied()
                .map(|module| package_root(module).to_owned())
                .collect::<HashSet<_>>();
            let mut undeclared = modeled.difference(&declared).collect::<Vec<_>>();
            undeclared.sort();
            assert!(
                undeclared.is_empty(),
                "{} models {undeclared:?} but declares no contract for them; add an entry to {} \
                 (a hand-authored bundled overlay sets \"generated\": false)",
                dialect.id,
                source.display()
            );
        }
    }

    fn location(index: u64) -> Location {
        Location {
            path: "catalog-prose.tsx".into(),
            start_byte: index,
            end_byte: index + 1,
        }
    }

    /// Materializes every shared static-defect wording branch plus the
    /// catalog findings that consumed the old dialect prose helpers: a strict
    /// read, an owned write, an ownerless cleanup (the owner arm whose hint
    /// diverges most between the versions), and a pending async read (a table
    /// only the 2.0 catalog projects). Dynamic subjects use a reserved prefix
    /// so the API-name assertion below can distinguish user code quoted by a
    /// finding from catalog-owned advice.
    fn catalog_prose_program() -> Program {
        let defect_kinds = [
            StaticDefectKind::ReactiveObjectDestructure {
                source: "props".into(),
                component_props: true,
            },
            StaticDefectKind::ReactiveReadAfterAwait {
                accessor: "sampleAccessor".into(),
            },
            StaticDefectKind::ComponentReturnsConditionally,
            StaticDefectKind::PackageContractExportMissing {
                module: "sample-package".into(),
                export: "sampleExport".into(),
                reexported: false,
                site: solid_reactive_ir::ContractDefectSite::Import,
                admission_refusal: None,
            },
            StaticDefectKind::MissingEffectFunction,
            StaticDefectKind::ReactiveSourceUncaptured {
                source: "sampleAccessor".into(),
                callee: "sampleCallee".into(),
            },
            StaticDefectKind::ReactiveHandlerRead {
                attribute: "onClick".into(),
                expression: "sampleHandler".into(),
            },
            StaticDefectKind::UncalledAccessor {
                name: "sampleAccessor".into(),
                position: "sample expression".into(),
            },
            StaticDefectKind::DirectMutation {
                name: "sampleAccessor".into(),
                target: DirectMutationTarget::AccessorBinding,
            },
            StaticDefectKind::DirectMutation {
                name: "sampleStore".into(),
                target: DirectMutationTarget::Store,
            },
            StaticDefectKind::DirectMutation {
                name: "sampleProps".into(),
                target: DirectMutationTarget::Props,
            },
            StaticDefectKind::DirectMutation {
                name: "sampleValue".into(),
                target: DirectMutationTarget::ReactiveValue,
            },
        ];
        Program {
            reads: vec![ReactiveRead {
                kind: "signal".into(),
                accessor: "sampleAccessor".into(),
                location: location(1),
                declaration: location(2),
                execution: ExecutionRole::UntrackedRendering,
                context: "sample component".into(),
                via: "".into(),
                origin: None,
                origin_context: "".into(),
                uncertain: false,
                missing_jsx_census: false,
            }],
            writes: vec![ReactiveWrite {
                setter: "sampleSetter".into(),
                operation: ReactiveWriteOperation::Setter,
                source_kind: solid_reactive_ir::ReactiveSourceKind::Accessor,
                location: location(3),
                declaration: location(4),
                execution: ExecutionRole::TrackedJsx,
                allowed_by_option: false,
                context: "sample computation".into(),
            }],
            missing_owners: vec![OwnerRequirement {
                operation: OwnerRequirementOperation::Cleanup,
                location: location(5),
                uncertain: false,
                runtime_uncertain: false,
                caller_uncertain: false,
                conditional_owner: false,
                later_run_unowned: false,
                component_uncertain: false,
                missing_jsx_census: false,
                report: true,
            }],
            async_reads: vec![AsyncRead {
                accessor: "sampleAsyncAccessor".into(),
                location: location(6),
                declaration: location(7),
                execution: ExecutionRole::TrackedJsx,
                leaf_owner: None,
                under_loading: false,
                async_provenance: true,
                declared_loading: false,
                options_opaque: false,
                ssr_client_hole: false,
                server_rendering_unresolved: false,
            }],
            static_defects: defect_kinds
                .into_iter()
                .enumerate()
                .map(|(index, kind)| StaticDefect {
                    kind,
                    location: location(index as u64 + 10),
                    analysis_context: String::new(),
                    fixes: vec![],
                    uncertain: false,
                })
                .collect(),
            ..Program::default()
        }
    }

    fn called_names(text: &str) -> impl Iterator<Item = String> + '_ {
        text.match_indices('(').filter_map(|(index, _)| {
            let name = text[..index]
                .chars()
                .rev()
                .take_while(|character| {
                    character.is_alphanumeric() || matches!(character, '_' | '$')
                })
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<String>();
            (!name.is_empty()).then_some(name)
        })
    }

    fn contains_identifier(text: &str, expected: &str) -> bool {
        text.split(|character: char| {
            !(character.is_alphanumeric() || matches!(character, '_' | '$'))
        })
        .any(|identifier| identifier == expected)
    }

    /// The default is read from the registry, not named.
    ///
    /// With one dialect this cannot fail, which is exactly why it is written
    /// as a property rather than as `assert_eq!(default_dialect().id,
    /// "solid-v2")`: the assertion that would have to change on the day a
    /// newer dialect is added is the assertion that would be wrong that day.
    #[test]
    fn the_default_is_the_newest_dialect_the_registry_carries() {
        let newest = ALL
            .iter()
            .map(|dialect| dialect.vocabulary.version())
            .max()
            .expect("ALL is non-empty");
        assert_eq!(default_dialect().vocabulary.version(), newest);
        assert!(
            ALL.iter().any(|dialect| dialect.id == default_dialect().id),
            "the default must be a registered dialect, not a value beside the registry"
        );
    }

    #[test]
    fn dialect_ids_are_unique_and_resolvable() {
        for dialect in ALL {
            assert_eq!(by_id(dialect.id).map(|found| found.id), Some(dialect.id));
        }
        assert!(by_id("solid-v3").is_none());
    }

    #[test]
    fn compatibility_registry_counts_match_the_catalog_migration() {
        assert_eq!(
            RULE_ALIASES.len(),
            19,
            "the migration note must list every transferred configuration key"
        );
        assert_eq!(
            RETIRED_RULES.len(),
            57,
            "eight pre-existing TypeScript redundancies, 31 catalog-reduction identities, and the 18 the 1.x catalog still declared when ADR 0110 deleted it"
        );
    }

    #[test]
    fn every_catalog_identity_resolves_to_its_metadata() {
        for dialect in ALL {
            if dialect.id == "solid-v2" {
                for rule in solid_v2_rules::Rule::ALL {
                    assert_eq!(
                        (dialect.rule_metadata)(rule.metadata().name),
                        Some(rule.metadata())
                    );
                }
            }
        }
    }

    /// Catalogs own user-facing wording, but the generated export index still
    /// owns the fact of which APIs exist. Exercise the real findings so moving
    /// prose out of [`solid_dialect::Dialect`] cannot also remove the guard
    /// that stopped 2.0 advice leaking into 1.x diagnostics (and vice versa).
    #[test]
    fn rule_catalog_prose_only_names_apis_exported_by_its_dialect() {
        const NON_SOLID_CALLS: &[&str] =
            &["dispose", "log", "queueMicrotask", "setStore", "setTimeout"];
        let program = catalog_prose_program();
        for (version, forbidden) in [
            (
                solid_dialect::Version::V1,
                &["action", "actions", "onSettled", "ownedWrite"][..],
            ),
            (
                solid_dialect::Version::V2,
                &[
                    "onMount",
                    "mergeProps",
                    "splitProps",
                    "produce",
                    "Suspense",
                    "SuspenseList",
                ][..],
            ),
        ] {
            // A single-dialect feature build simply has nothing to check for
            // the absent version.
            let Some(dialect) = by_version(version) else {
                continue;
            };
            let findings = dialect.solve(&program);
            // Beyond the defects: the strict read, the owned write, and the
            // ownerless cleanup. The pending async read joins them only in
            // 2.0 — the 1.x catalog deliberately never reads that table.
            let catalog_findings = match version {
                solid_dialect::Version::V1 => 3,
                solid_dialect::Version::V2 => 4,
            };
            assert_eq!(
                findings.len(),
                program.static_defects.len() + catalog_findings
            );

            let mut checked_calls = 0;
            let mut prose = String::new();
            for finding in &findings {
                for (field, text) in std::iter::once(("message", finding.message.as_str()))
                    .chain(std::iter::once(("hint", finding.hint.as_str())))
                    .chain(
                        finding
                            .evidence
                            .iter()
                            .map(|step| ("evidence", step.message.as_str())),
                    )
                {
                    prose.push_str(text);
                    prose.push('\n');
                    for name in called_names(text) {
                        if name.starts_with("sample") || NON_SOLID_CALLS.contains(&name.as_str()) {
                            continue;
                        }
                        assert!(
                            !dialect
                                .vocabulary
                                .export_modules(&name, solid_dialect::ExportPosition::Value)
                                .is_empty(),
                            "{version:?} catalog {field} names {name}(), which that dialect does not export: {text:?}"
                        );
                        checked_calls += 1;
                    }
                }
            }
            assert!(
                checked_calls >= 5,
                "{version:?}: only {checked_calls} API calls checked"
            );
            for name in forbidden {
                assert!(
                    !contains_identifier(&prose, name),
                    "{version:?} catalog names the other dialect's {name}: {prose}"
                );
            }

            match version {
                solid_dialect::Version::V1 => {
                    assert!(prose.contains("createEffect(fn, value?)"));
                    assert!(prose.contains("splitProps(props"));
                    assert!(prose.contains("mergeProps(defaults"));
                    assert_eq!(
                        dialect
                            .vocabulary
                            .callback_positions(solid_dialect::Primitive::CreateEffect),
                        &[0]
                    );
                }
                solid_dialect::Version::V2 => {
                    assert!(prose.contains("createEffect(compute, apply)"));
                    assert!(prose.contains("omit(props"));
                    assert!(prose.contains("merge(defaults"));
                    assert_eq!(
                        dialect
                            .vocabulary
                            .callback_positions(solid_dialect::Primitive::CreateEffect),
                        &[1]
                    );
                }
            }
        }
    }

    #[test]
    fn detection_reads_the_resolved_solid_js_version() {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-dialect-detect-{}",
            std::process::id()
        ));
        let package = root.join("node_modules/solid-js");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"solid-js","version":"1.9.14"}"#,
        )
        .unwrap();
        let project = root.join("src/tsconfig.json");
        std::fs::write(&project, "{}").unwrap();
        // The classification, not the dialect id: a 1.x install resolves to
        // `Version::V1` in every build, and only a build carrying the 1.x
        // dialect can turn that into one.
        assert!(matches!(
            detect_detailed(&project),
            Detection::Installed {
                version: solid_dialect::Version::V1,
                ..
            } | Detection::Unsupported {
                classification: solid_dialect::Classification::Modelled(solid_dialect::Version::V1,),
                ..
            }
        ));

        std::fs::write(
            package.join("package.json"),
            r#"{"name":"solid-js","version":"2.0.0-rc.0"}"#,
        )
        .unwrap();
        assert!(matches!(
            detect_detailed(&project),
            Detection::Installed {
                version: solid_dialect::Version::V2,
                ..
            } | Detection::Unsupported {
                classification: solid_dialect::Classification::Modelled(solid_dialect::Version::V2,),
                ..
            }
        ));

        // No resolvable version answers the default rather than guessing.
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"solid-js","version":"workspace:*"}"#,
        )
        .unwrap();
        assert!(matches!(
            detect_detailed(&project),
            Detection::Defaulted { manifest: Some(_) }
        ));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_broken_nearer_manifest_does_not_mask_an_installation_higher_up() {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-dialect-broken-stub-{}",
            std::process::id()
        ));
        let outer = root.join("node_modules/solid-js");
        let inner = root.join("workspace/node_modules/solid-js");
        std::fs::create_dir_all(&outer).unwrap();
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::create_dir_all(root.join("workspace/src")).unwrap();
        std::fs::write(
            outer.join("package.json"),
            r#"{"name":"solid-js","version":"1.9.14"}"#,
        )
        .unwrap();
        let project = root.join("workspace/src/tsconfig.json");
        std::fs::write(&project, "{}").unwrap();

        // Unparseable JSON and a version-less manifest are both the walk
        // continuing, exactly like an unreadable file.
        //
        // Asserted as *which manifest the walk selected*, which is the claim.
        // Reading it off a dialect id instead made this test depend on the 1.x
        // dialect being compiled in, and it failed in the
        // `--features dialect-v2` arm for a reason that has nothing to do with
        // the walk.
        let selected = |detection: Detection| match detection {
            Detection::Installed { manifest, .. } | Detection::Unsupported { manifest, .. } => {
                manifest
            }
            Detection::Defaulted { manifest } => {
                panic!("the outer 1.x install is a resolution, not a default: {manifest:?}")
            }
        };
        std::fs::write(inner.join("package.json"), "{ not json").unwrap();
        assert_eq!(
            selected(detect_detailed(&project)),
            outer.join("package.json")
        );
        std::fs::write(inner.join("package.json"), r#"{"name":"solid-js"}"#).unwrap();
        assert_eq!(
            selected(detect_detailed(&project)),
            outer.join("package.json")
        );

        // A parseable version that classifies stops the walk at the nearest
        // manifest, masking the outer 1.x -- resolution order, not breakage.
        std::fs::write(
            inner.join("package.json"),
            r#"{"name":"solid-js","version":"2.0.0-rc.0"}"#,
        )
        .unwrap();
        assert_eq!(
            selected(detect_detailed(&project)),
            inner.join("package.json")
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The three outcomes `detect` collapses, kept apart -- and the manifest
    /// path a refusal has to name. The walk is unbounded, so the deciding
    /// `package.json` is frequently not the one beside the project, and a
    /// refusal that cannot say which file decided it is not actionable.
    #[test]
    fn detection_separates_an_install_from_a_default_and_names_the_manifest() {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-dialect-detection-{}",
            std::process::id()
        ));
        let package = root.join("node_modules/solid-js");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        let manifest = package.join("package.json");
        let project = root.join("src/tsconfig.json");
        std::fs::write(&project, "{}").unwrap();

        std::fs::write(&manifest, r#"{"name":"solid-js","version":"2.0.0-rc.0"}"#).unwrap();
        match detect_detailed(&project) {
            Detection::Installed {
                dialect,
                version,
                manifest: read,
                ..
            } => {
                assert_eq!(dialect.id, "solid-v2");
                assert_eq!(version, solid_dialect::Version::V2);
                // Named from two directories up, not from beside the project.
                assert_eq!(read, manifest);
            }
            // A build without the 2.0 dialect still resolves the install and
            // still names the file; only the dialect is missing.
            Detection::Unsupported {
                classification: solid_dialect::Classification::Modelled(solid_dialect::Version::V2),
                installed,
                manifest: read,
                ..
            } => {
                assert_eq!(read, manifest);
                assert_eq!(installed, "2.0.0-rc.0");
            }
            other => panic!("an installed 2.0 is a resolution: {other:?}"),
        }

        // Resolvable but unclassifiable: a default that can still say which
        // file it read, which is what separates it from nothing installed.
        std::fs::write(&manifest, r#"{"name":"solid-js","version":"workspace:*"}"#).unwrap();
        match detect_detailed(&project) {
            Detection::Defaulted {
                manifest: Some(read),
            } => assert_eq!(read, manifest),
            other => panic!("an unclassifiable version defaults, and names its file: {other:?}"),
        }

        // Nothing installed anywhere above: a default with no file to name.
        std::fs::remove_dir_all(root.join("node_modules")).unwrap();
        assert!(matches!(
            detect_detailed(&project),
            Detection::Defaulted { manifest: None }
        ));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The refusal's identity is held in two places on purpose; this is what
    /// stops them drifting.
    ///
    /// [`UNSUPPORTED_RUNTIME_CODE`] is what the emission actually writes, and
    /// it cannot read a catalog because the refusal happens before a dialect
    /// is chosen. The Solid 2 catalog is where every *consumer* resolves the
    /// identity -- the npm rules manifest, suppression configuration, the
    /// docs URL -- so a disagreement between them would publish a code no
    /// adapter recognizes while the tool emitted it anyway.
    #[cfg(feature = "dialect-v2")]
    #[test]
    fn the_refusal_identity_is_the_one_the_catalog_publishes() {
        let dialect = by_id("solid-v2").expect("the 2.0 dialect is compiled in");
        let metadata = (dialect.rule_metadata)(UNSUPPORTED_RUNTIME_RULE)
            .unwrap_or_else(|| panic!("the 2.0 catalog must declare {UNSUPPORTED_RUNTIME_RULE}"));
        assert_eq!(metadata.code, UNSUPPORTED_RUNTIME_CODE);
        assert_eq!(metadata.name, UNSUPPORTED_RUNTIME_RULE);
        assert!(
            metadata.uncertifiable,
            "the refusal asserts nothing about the project's source, so it is an \
             uncertifiable result and not a violation"
        );
        assert_eq!(metadata.severity, "error");
    }

    /// The refusal snapshot is the *whole* result, and its shape is the claim.
    #[test]
    fn the_refusal_snapshot_carries_one_finding_and_measures_nothing() {
        let snapshot = crate::diagnostics::unsupported_runtime_snapshot(
            "1.9.14",
            Path::new("/tmp/app/node_modules/solid-js/package.json"),
            None,
        );
        assert_eq!(snapshot.status, "uncertifiable");
        assert_eq!(
            snapshot.findings.len(),
            1,
            "a second finding would assert something about source that was \
             never analyzed under the language it runs"
        );
        let finding = &snapshot.findings[0];
        assert_eq!(finding.id, UNSUPPORTED_RUNTIME_CODE);
        assert_eq!(finding.rule, UNSUPPORTED_RUNTIME_RULE);
        assert_eq!(finding.kind, "uncertifiable");
        assert!(
            finding.message.contains("1.9.14"),
            "the refusal quotes the version it read, not the classified major: {}",
            finding.message
        );
        assert_eq!(
            finding.primary_location.path, "/tmp/app/node_modules/solid-js/package.json",
            "the deciding manifest is the location, because it is the file to change"
        );
        assert_eq!(
            snapshot.metrics.files_analyzed, 0,
            "nothing was read; reporting otherwise would overstate what happened"
        );
        assert!(snapshot.package_summaries.is_empty());
    }

    /// A named major with no vocabulary behind it refuses.
    ///
    /// This is the half of the refusal that `Version` can still express:
    /// `V1` exists precisely so detection can *recognise* `1.9.14` in order to
    /// turn it down. The other half -- a major no variant names at all --
    /// classifies as `UnmodelledMajor` and is pinned end to end by
    /// `a_major_this_build_does_not_name_is_refused_like_one_it_does` in
    /// `tests/dialects_process.rs`, because its whole claim is about what the
    /// process emits.
    #[test]
    fn a_named_major_with_no_dialect_behind_it_is_unsupported() {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-dialect-unsupported-{}",
            std::process::id()
        ));
        let package = root.join("node_modules/solid-js");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"solid-js","version":"1.9.14"}"#,
        )
        .unwrap();
        let project = root.join("src/tsconfig.json");
        std::fs::write(&project, "{}").unwrap();

        let detection = detect_detailed(&project);
        assert!(
            matches!(
                &detection,
                Detection::Unsupported {
                    classification: solid_dialect::Classification::Modelled(
                        solid_dialect::Version::V1,
                    ),
                    ..
                }
            ),
            "a 1.x install is unsupported, not a 2.0 project: {detection:?}"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A scratch project whose nearest `node_modules/solid-js` names `version`.
    fn installed_project(tag: &str, version: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "solid-checker-dialect-release-{tag}-{}",
            std::process::id()
        ));
        let package = root.join("node_modules/solid-js");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            package.join("package.json"),
            format!(r#"{{"name":"solid-js","version":"{version}"}}"#),
        )
        .unwrap();
        let project = root.join("src/tsconfig.json");
        std::fs::write(&project, "{}").unwrap();
        (root, project)
    }

    /// The release classification end to end through detection: which dialect
    /// analyzes each release, and whether the analysis carries the notice.
    #[cfg(feature = "dialect-v2")]
    #[test]
    fn detection_classifies_the_installed_release_within_the_major() {
        // (installed, selected dialect id, notice due)
        let rows = [
            ("2.0.0-rc.3", "solid-v2", false),
            ("2.0.0-rc.0", "solid-v2", false),
            ("2.0.0-rc.9", "solid-v2@2.0.0-rc.9", true),
            ("2.0.0-rc.6", "solid-v2", true),
            ("2.0.0-rc.10", "solid-v2", true),
            ("2.0.0-beta.19", "solid-v2", true),
            ("2.0.0", "solid-v2", true),
        ];
        for (index, (installed, id, notice_due)) in rows.into_iter().enumerate() {
            let (root, project) = installed_project(&format!("row{index}"), installed);
            let detection = detect_detailed(&project);
            let Detection::Installed {
                dialect,
                installed: read,
                ..
            } = &detection
            else {
                panic!("{installed} is analyzed: {detection:?}");
            };
            assert_eq!(dialect.id, id, "{installed}");
            assert_eq!(read, installed);
            let notice = release_notice(dialect, &project);
            assert_eq!(notice.is_some(), notice_due, "{installed}: {notice:?}");
            if let Some(notice) = notice {
                assert_eq!(notice.installed, installed);
                assert_eq!(
                    notice.manifest,
                    root.join("node_modules/solid-js/package.json")
                );
            }
            // A dialect detection did not pick is a decision: no notice. The
            // audited language under an rc.9 install is the `--dialect
            // solid-v2` escape hatch.
            if dialect.id != SOLID_V2.id {
                assert_eq!(release_notice(&SOLID_V2, &project), None, "{installed}");
            }
            std::fs::remove_dir_all(&root).unwrap();
        }
    }

    /// The pre-beta experiment is refused although its major is carried, and
    /// the refusal carries the vocabulary's reason.
    #[cfg(feature = "dialect-v2")]
    #[test]
    fn the_pre_beta_experiment_is_refused_with_its_reason() {
        let (root, project) = installed_project("experimental", "2.0.0-experimental.1");
        let detection = detect_detailed(&project);
        let Detection::Unsupported {
            classification,
            installed,
            refusal: Some(refusal),
            manifest,
        } = &detection
        else {
            panic!("2.0.0-experimental.1 is refused: {detection:?}");
        };
        assert_eq!(
            *classification,
            solid_dialect::Classification::Modelled(solid_dialect::Version::V2)
        );
        assert_eq!(installed, "2.0.0-experimental.1");
        assert_eq!(refusal.line, "2.0.0-experimental.x");
        let snapshot =
            crate::diagnostics::unsupported_runtime_snapshot(installed, manifest, Some(refusal));
        assert_eq!(snapshot.status, "uncertifiable");
        assert_eq!(snapshot.findings.len(), 1);
        let message = &snapshot.findings[0].message;
        assert!(
            message.contains("2.0.0-experimental.1")
                && message.contains("argument 2")
                && !message.contains("carries no dialect"),
            "the refusal states the vocabulary's reason, not an absent dialect: {message}"
        );
        assert_eq!(snapshot.findings[0].id, UNSUPPORTED_RUNTIME_CODE);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// Every variant is the release its language's vocabulary reviews, named
    /// after it, resolvable by id, and absent from the language registry.
    #[cfg(feature = "dialect-v2")]
    #[test]
    fn every_release_variant_is_the_release_its_review_names() {
        for (release, variant) in RELEASE_VARIANTS {
            let language = ALL
                .iter()
                .copied()
                .find(|language| language.vocabulary.version() == variant.vocabulary.version())
                .expect("a variant belongs to a registered language");
            let solid_dialect::ReleaseReview::ReviewedWithGaps(reviewed) =
                language.vocabulary.review_release(release)
            else {
                panic!("{release} has a variant but its language does not review it");
            };
            assert_eq!(reviewed.version, *release);
            assert_eq!(variant.id, format!("{}@{release}", language.id));
            assert_eq!(by_id(variant.id).map(|found| found.id), Some(variant.id));
            assert!(!ALL.iter().any(|registered| registered.id == variant.id));
            // The variant is the same catalog; only the vocabulary moves.
            assert_eq!(variant.rule_count, language.rule_count);
            assert_eq!(
                variant.compiler_facts_identity,
                language.compiler_facts_identity
            );
        }
        // B1, as the two dialects detection selects answer it.
        assert!(SOLID_V2.vocabulary.store_root_properties_are_readonly());
        assert!(!SOLID_V2_RC9.vocabulary.store_root_properties_are_readonly());
    }

    /// The notice's identity is held here and published by the catalog; this
    /// keeps them from drifting, as for the refusal.
    #[cfg(feature = "dialect-v2")]
    #[test]
    fn the_notice_identity_is_the_one_the_catalog_publishes() {
        let dialect = by_id("solid-v2").expect("the 2.0 dialect is compiled in");
        let metadata = (dialect.rule_metadata)(UNAUDITED_RELEASE_RULE)
            .unwrap_or_else(|| panic!("the 2.0 catalog must declare {UNAUDITED_RELEASE_RULE}"));
        assert_eq!(metadata.code, UNAUDITED_RELEASE_CODE);
        assert!(metadata.uncertifiable);
        let notice = ReleaseNotice {
            installed: "2.0.0-rc.9".into(),
            manifest: PathBuf::from("/tmp/app/node_modules/solid-js/package.json"),
            review: solid_dialect::Dialect::review_release(&solid_dialect::Solid2, "2.0.0-rc.9"),
        };
        let finding = crate::diagnostics::unaudited_release_finding(&notice);
        assert_eq!(finding.id, metadata.code);
        assert_eq!(finding.rule, metadata.name);
        assert_eq!(finding.severity, metadata.severity);
        assert_eq!(finding.kind, "uncertifiable");
        assert_eq!(finding.subject_kind, "project");
        assert!(
            finding.message.contains("2.0.0-rc.9"),
            "{}",
            finding.message
        );
        assert!(
            !finding.message.contains("  ") && !finding.hint.contains("  "),
            "wording is one line of prose: {:?} / {:?}",
            finding.message,
            finding.hint
        );
        let unreviewed = crate::diagnostics::unaudited_release_finding(&ReleaseNotice {
            installed: "2.0.0-rc.6".into(),
            manifest: notice.manifest.clone(),
            review: solid_dialect::ReleaseReview::Unreviewed,
        });
        assert!(
            unreviewed
                .message
                .contains("has not been reviewed against 2.0.0-rc.6")
                && !unreviewed.message.contains("  ")
                && !unreviewed.hint.contains("  "),
            "{:?} / {:?}",
            unreviewed.message,
            unreviewed.hint
        );
        assert_eq!(unreviewed.evidence.len(), 1, "the manifest step only");
        assert!(
            finding
                .hint
                .contains("2026-09-26-solid-2-rc9-vocabulary-review.md"),
            "the notice points at the review: {}",
            finding.hint
        );
        let gaps = finding
            .evidence
            .iter()
            .filter(|step| step.message.starts_with("known gap "))
            .count();
        let solid_dialect::ReleaseReview::ReviewedWithGaps(reviewed) = notice.review else {
            panic!("rc.9 is reviewed with gaps");
        };
        assert_eq!(gaps, reviewed.known_gaps.len(), "every known gap is named");
        assert_eq!(
            &*finding.primary_location.path,
            "/tmp/app/node_modules/solid-js/package.json"
        );
    }
}
