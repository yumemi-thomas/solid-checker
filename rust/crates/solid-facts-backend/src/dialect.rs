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

/// The note for every former name of SC9005 (ADR 0440).
const RETIRED_AS_OBLIGATION_SC9005: &str = "became an obligation 2026-10-26: required package behavior unavailable from source (SC9005) is an internal uncertifiable result, not a rule, and no option disables it (ADR 0440); the key does nothing";

/// The note for SC9012 (ADR 0440).
const RETIRED_AS_OBLIGATION_SC9012: &str = "became an obligation 2026-10-26: an unresolved reactive dispatch (SC9012) is an internal uncertifiable result, not a rule, and no option disables it (ADR 0440); the key does nothing";

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
    // ADR 0440: SC9005, SC9011 and SC9012 are obligations, internal
    // uncertifiable results, not rules. A configuration that named one still
    // loads, and the key does nothing: no option reaches an obligation.
    ("package-behavior-unavailable", RETIRED_AS_OBLIGATION_SC9005),
    ("package-contract-incomplete", RETIRED_AS_OBLIGATION_SC9005),
    (
        "package-contract-export-missing",
        RETIRED_AS_OBLIGATION_SC9005,
    ),
    ("package-contract-missing", RETIRED_AS_OBLIGATION_SC9005),
    (
        "package-contract-callback-missing",
        RETIRED_AS_OBLIGATION_SC9005,
    ),
    ("reactive-dispatch-unresolved", RETIRED_AS_OBLIGATION_SC9012),
    (
        "reactive-source-uncaptured",
        "merged 2026-10-26 into SC9012 as the context `package-callee`, which is an obligation and not a rule (ADR 0440); the key does nothing",
    ),
];

/// Diagnostic codes that no longer exist (ADR 0440): SC9011 merged into SC9012.
/// A `solid-checker-*` suppression comment may still name one; it is accepted
/// silently and suppresses nothing.
pub const RETIRED_CODES: &[&str] = &["SC9011"];

/// Whether `code` is a diagnostic code the catalog retired.
#[must_use]
pub fn retired_code(code: &str) -> bool {
    RETIRED_CODES.contains(&code)
}

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
    // ADR 0381: catalog names follow eslint-plugin-solid where it still
    // describes what the checker proves.
    (
        "reactive-write-in-owned-scope",
        "no-write-in-pure-computation",
    ),
    ("no-direct-mutation", "no-store-mutation-outside-setter"),
    // ADR 0426: `resolve` and `until` are one rule, `SC2004` and `SC2005`.
    (
        "resolve-in-tracked-scope",
        "no-await-primitive-in-tracked-scope",
    ),
    (
        "resolve-in-reactive-scope",
        "no-await-primitive-in-tracked-scope",
    ),
    (
        "until-in-tracked-scope",
        "no-await-primitive-in-tracked-scope",
    ),
    ("server-function-module-directive", "valid-use-server"),
    (
        "sync-node-received-async",
        "sync-computation-received-async",
    ),
];

/// Former names that covered one code of a merged rule (ADR 0426): configuring
/// one reaches that code alone, where the current name reaches every code of
/// the rule. Each name here is also in [`RULE_ALIASES`], which validates it.
pub const CODE_SCOPED_ALIASES: &[(&str, &str)] = &[
    ("resolve-in-tracked-scope", "SC2004"),
    ("resolve-in-reactive-scope", "SC2004"),
    ("until-in-tracked-scope", "SC2005"),
];

/// Group keys (ADR 0428): one name that configures several rules at once, as
/// eslint-plugin-solid's `reactivity` does for the four defects it reports. A
/// group is not a rule: it has no finding, no code and no ESLint severity, and
/// an explicit setting of a member wins over the group's.
pub const RULE_GROUPS: &[(&str, &[&str])] = &[(
    "reactivity",
    &[
        "strict-read-untracked",
        "reactive-read-after-await",
        "uncalled-accessor",
        "reactive-handler-frozen",
    ],
)];

/// The rule names eslint-plugin-solid v0.18.1 registers (`allRules` in
/// `src/plugin.ts`; `validate-jsx-nesting` is commented out there). A
/// suppression comment naming `solid/<name>` for one of them is accepted
/// whether or not this checker has a rule of that name (ADR 0429); any other
/// `solid/*` name belongs to whatever plugin owns that namespace.
pub const UPSTREAM_RULE_NAMES: &[&str] = &[
    "components-return-once",
    "event-handlers",
    "imports",
    "jsx-no-duplicate-props",
    "jsx-no-undef",
    "jsx-no-script-url",
    "jsx-uses-vars",
    "no-accessor-as-prop",
    "no-boolean-enumerated-attribute",
    "no-browser-globals-in-server-function",
    "no-destructure",
    "no-innerhtml",
    "no-invalid-server-capture",
    "no-module-scope-reactive-primitive",
    "no-proxy-apis",
    "no-react-deps",
    "no-react-specific-props",
    "no-restated-default-options",
    "no-single-arg-create-effect",
    "no-store-mutation-outside-setter",
    "no-unknown-namespaces",
    "no-unused-signal",
    "no-write-in-pure-computation",
    "prefer-classlist",
    "prefer-for",
    "prefer-onSettled-for-side-effects",
    "prefer-show",
    "prefer-structured-class",
    "reactivity",
    "removed-api",
    "require-async-server-function",
    "self-closing-comp",
    "style-prop",
    "valid-use-server",
    "no-array-handlers",
];

/// The ESLint adapter's own rule ids, which a project writes in disable
/// comments but which report no finding of their own.
pub const ADAPTER_RULE_IDS: &[&str] = &["certification", "package-note", "contract-note"];

/// The member rules of a group key, if `name` is one.
#[must_use]
pub fn rule_group(name: &str) -> Option<&'static [&'static str]> {
    RULE_GROUPS
        .iter()
        .find(|(group, _)| *group == name)
        .map(|(_, members)| *members)
}

/// The finding code a former rule name is limited to, if it is.
#[must_use]
pub fn rule_alias_code(name: &str) -> Option<&'static str> {
    CODE_SCOPED_ALIASES
        .iter()
        .find(|(old, _)| *old == name)
        .map(|(_, code)| *code)
}

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
    /// ADR 0436: the declared key set of a spread argument on an intrinsic
    /// element, which `missing-owner` asks to prove the spread holds no
    /// listener key.
    pub spread_declared_keys: bool,
}

impl SemanticDemandCapabilities {
    pub const NONE: Self = Self {
        array_map_receiver_types: false,
        async_array_map_callbacks: false,
        server_argument_library_types: false,
        spread_declared_keys: false,
    };
    /// Only the 2.0 catalog carries `server-function-rich-argument`, so only it
    /// pays for the library-type identities that rule reads.
    const SOLID_2: Self = Self {
        array_map_receiver_types: true,
        async_array_map_callbacks: true,
        server_argument_library_types: true,
        spread_declared_keys: true,
    };
}

#[derive(Clone, Copy)]
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
    /// Whether the catalog carries a *configurable* rule with this externally
    /// visible name: the names a rule-options document, an alias and the
    /// plugin manifest may use. An obligation (SC9005, SC9012; ADR 0440) has
    /// metadata but no key, so it is not one. Lets shared backend code
    /// condition work on catalog capability (for example, which type facts to
    /// demand) instead of naming a version.
    pub has_rule: fn(&str) -> bool,
    /// Whether the catalog declares a diagnostic with this code.
    pub has_code: fn(&str) -> bool,
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

/// Release variants of the registered dialects: one per vocabulary a
/// language's [`solid_dialect::Dialect::review_installation`] can answer an
/// installation with, other than the language's own.
///
/// Not in [`ALL`]: a variant is not a language. It is the same catalog,
/// compiler and runtime model with a vocabulary that answers some questions
/// differently for the installed releases (the rc.9 and rc.1-rc.8 reviews: the
/// store root typing and `omit`'s predicate form from `@solidjs/signals`,
/// `until`, and `dynamic`'s options from `@solidjs/web`). It has an id of its own, `<language>@<variant key>`,
/// because the id keys every cache, retained session and daemon socket, and a
/// result computed under one vocabulary must never answer for another;
/// [`by_id`] resolves it so the daemon can forward the selection it hashed.
/// The key is spelled from the vocabulary's answers, so moving the audited
/// release renames no variant.
///
/// Registered on first use, keyed by id: a process reaches one or two of a
/// language's variants, and a language can answer with well over a hundred
/// thousand, so neither an id nor a [`Dialect`] is built for one nobody
/// reaches. A variant once registered is the same `&'static` for the rest of
/// the process.
static RELEASE_VARIANTS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, &'static Dialect>>,
> = std::sync::LazyLock::new(Default::default);

/// Resolves a dialect by its stable id: a registered language, or one of its
/// release variants, whose key is decoded rather than looked up
/// ([`solid_dialect::Dialect::variant_by_key`]).
#[must_use]
pub fn by_id(id: &str) -> Option<&'static Dialect> {
    if let Some(language) = ALL.iter().copied().find(|dialect| dialect.id == id) {
        return Some(language);
    }
    let (language, key) = id.split_once('@')?;
    let language = ALL.iter().copied().find(|dialect| dialect.id == language)?;
    let vocabulary = language.vocabulary.variant_by_key(key)?;
    Some(for_vocabulary(language, Some(vocabulary)))
}

/// The dialect that analyzes with `vocabulary`, the one `language`'s own
/// review chose: the language itself, or its release variant, registered on
/// first use ([`RELEASE_VARIANTS`]).
fn for_vocabulary(
    language: &'static Dialect,
    vocabulary: Option<&'static dyn solid_dialect::Dialect>,
) -> &'static Dialect {
    let Some(key) = vocabulary.and_then(|vocabulary| vocabulary.variant_key()) else {
        return language;
    };
    let vocabulary = language
        .vocabulary
        .variant_by_key(key)
        .filter(|variant| variant.version() == language.vocabulary.version())
        .expect("a language's review answers only with the variants it lists");
    let id = format!("{}@{key}", language.id);
    let mut registry = RELEASE_VARIANTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(variant) = registry.get(&id) {
        return variant;
    }
    let variant: &'static Dialect = Box::leak(Box::new(Dialect {
        id: id.clone().leak(),
        vocabulary,
        ..*language
    }));
    registry.insert(id, variant);
    variant
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
///
/// Within that language, the vocabulary is the one it names for a project
/// with nothing installed ([`solid_dialect::Dialect::defaulted_vocabulary`]):
/// the language itself, or one of its [`RELEASE_VARIANTS`].
#[must_use]
pub fn default_dialect() -> &'static Dialect {
    let language = ALL
        .iter()
        .copied()
        .max_by_key(|dialect| dialect.vocabulary.version())
        .expect("the const assertion above holds ALL non-empty");
    for_vocabulary(language, language.vocabulary.defaulted_vocabulary())
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

/// The diagnostic identity of the notice for a file the dialect compiler
/// refused (ADR 0360). Fact building decides it, not a rule, and it is pinned
/// to the catalog like [`UNAUDITED_RELEASE_CODE`].
pub const COMPILER_REFUSED_CODE: &str = "SC9015";
/// The rule name paired with [`COMPILER_REFUSED_CODE`].
pub const COMPILER_REFUSED_RULE: &str = "compiler-refused-source";

/// The diagnostic identity of the notice for a suppression comment that names
/// no rule of this checker (ADR 0429). Suppression parsing decides it, not a
/// rule, and it is pinned to the catalog like [`COMPILER_REFUSED_CODE`].
pub const UNKNOWN_SUPPRESSION_CODE: &str = "SC9016";
/// The rule name paired with [`UNKNOWN_SUPPRESSION_CODE`].
pub const UNKNOWN_SUPPRESSION_RULE: &str = "unknown-suppression-rule";

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
        /// The dialect that analyzes this installation: the language, or the
        /// [`RELEASE_VARIANTS`] entry its review chose.
        dialect: &'static Dialect,
        version: solid_dialect::Version,
        manifest: PathBuf,
        /// The `version` field exactly as the manifest spelled it.
        installed: String,
        /// Every package the language names as deciding a release-dependent
        /// answer ([`solid_dialect::Dialect::release_owners`]), as resolved.
        releases: Vec<ResolvedRelease>,
        /// The language vocabulary's open gaps for that installation. Any gap
        /// means the analysis carries the `SC9014` notice
        /// ([`release_notice`]).
        gaps: Vec<solid_dialect::InstallationGap>,
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
        /// release line ([`solid_dialect::InstallationReview::Refused`]) -- the
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
/// Deliberately **not** read from the checker's own vocabulary — it carries
/// the version the checker ships, not the one the project installed.
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
        // The major chose the language; the installed releases are the
        // vocabulary's to judge. Asked of the language's own vocabulary, so a
        // variant never reviews the installations that select it.
        let releases = resolved_releases(language.vocabulary, project, &manifest, &installed);
        let installation = releases
            .iter()
            .map(|release| solid_dialect::InstalledRelease {
                package: release.package,
                version: release.version.as_deref(),
            })
            .collect::<Vec<_>>();
        return match language.vocabulary.review_installation(&installation) {
            solid_dialect::InstallationReview::Refused(refusal) => Detection::Unsupported {
                classification,
                installed,
                manifest,
                refusal: Some(refusal),
            },
            solid_dialect::InstallationReview::Analyzed { vocabulary, gaps } => {
                Detection::Installed {
                    dialect: for_vocabulary(language, vocabulary),
                    version,
                    manifest,
                    installed,
                    releases,
                    gaps,
                }
            }
        };
    }
    Detection::Unsupported {
        classification,
        installed,
        manifest,
        refusal: None,
    }
}

/// One [`solid_dialect::ReleaseOwner`] as resolved for a project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedRelease {
    /// The owner's package name.
    pub package: &'static str,
    /// The resolved manifest's `version` field exactly as spelled; `None`
    /// when no manifest carrying a version string resolved.
    pub version: Option<String>,
    /// The manifest that resolved, if any.
    pub manifest: Option<PathBuf>,
}

/// An installation the analysis proceeds on without the vocabulary having
/// been audited on it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseNotice {
    /// The `solid-js` `version` field exactly as the manifest spelled it.
    pub installed: String,
    /// The `solid-js` manifest that selected the language; the notice is
    /// located there.
    pub manifest: PathBuf,
    /// Every release owner, as resolved.
    pub releases: Vec<ResolvedRelease>,
    /// The open gaps, never empty.
    pub gaps: Vec<solid_dialect::InstallationGap>,
    /// What to pin to certify ([`solid_dialect::Dialect::audited_installation`]).
    pub audited: &'static [(&'static str, &'static str)],
    /// Where the project reaches a scoped gap's exports, one site per
    /// distinct reach: why each scoped gap in `gaps` is due. Empty until the
    /// notice is scoped to a project's facts ([`ReleaseNotice::scoped_to`]).
    pub reaches: Vec<GapReach>,
}

/// One site that makes a scoped gap ([`solid_dialect::GapScope`]) due.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GapReach {
    pub message: String,
    pub path: String,
    pub span: solid_facts::core::Span,
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
///
/// This is the installation's notice: every gap, scoped ones included. What a
/// project's analysis carries is [`ReleaseNotice::scoped_to`] its facts, which
/// drops a scoped gap the project does not reach.
#[must_use]
pub fn release_notice(dialect: &'static Dialect, project: &Path) -> Option<ReleaseNotice> {
    let Detection::Installed {
        dialect: detected,
        version,
        manifest,
        installed,
        releases,
        gaps,
    } = detect_detailed(project)
    else {
        return None;
    };
    if detected.id != dialect.id || gaps.is_empty() {
        return None;
    }
    let audited = by_version(version).map_or(&[][..], |language| {
        language.vocabulary.audited_installation()
    });
    Some(ReleaseNotice {
        installed,
        manifest,
        releases,
        gaps,
        audited,
        reaches: Vec::new(),
    })
}

/// Every manifest the installation review read for `project`, resolved or
/// not: what a watcher must hash so that an install moving one owner between
/// releases does not keep serving the previous answer. An owner that did not
/// resolve contributes the path an install would most likely create, so
/// creating it is seen too.
#[must_use]
pub fn release_manifests(project: &Path) -> Vec<PathBuf> {
    let Some((Some(solid_dialect::Classification::Modelled(version)), installed, manifest)) =
        resolved_solid_version(project)
    else {
        return Vec::new();
    };
    let Some(language) = by_version(version) else {
        return Vec::new();
    };
    let mut paths = vec![manifest.clone()];
    for release in resolved_releases(language.vocabulary, project, &manifest, &installed) {
        if let Some(found) = release.manifest {
            paths.push(found);
        } else if let Some(node_modules) = manifest.parent().and_then(Path::parent) {
            paths.push(node_modules.join(release.package).join("package.json"));
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

/// Resolves every release owner `vocabulary` names for `project`, in order.
///
/// `solid-js` is the manifest detection already read. Every other owner is
/// the nearest `node_modules/<package>/package.json` carrying a version string
/// above where it resolves from: the project, or the *real* directory of the
/// owner it is resolved from. The real directory, because a package manager
/// that links packages (pnpm) places a package's own dependencies beside its
/// target, not beside the link; resolving from the link would read whatever
/// the project root happens to have.
fn resolved_releases(
    vocabulary: &dyn solid_dialect::Dialect,
    project: &Path,
    solid_js_manifest: &Path,
    solid_js_version: &str,
) -> Vec<ResolvedRelease> {
    let mut releases: Vec<ResolvedRelease> = Vec::new();
    for owner in vocabulary.release_owners() {
        let resolved = if owner.package == "solid-js" && owner.resolved_from.is_none() {
            Some((solid_js_version.to_owned(), solid_js_manifest.to_path_buf()))
        } else {
            let start = match owner.resolved_from {
                None => Some(project.to_path_buf()),
                Some(from) => releases
                    .iter()
                    .find(|release| release.package == from)
                    .and_then(|release| release.manifest.as_deref())
                    .and_then(Path::parent)
                    .map(|directory| {
                        std::fs::canonicalize(directory).unwrap_or_else(|_| directory.into())
                    }),
            };
            start.and_then(|start| nearest_package_version(&start, owner.package))
        };
        let (version, manifest) = resolved.map_or((None, None), |(version, manifest)| {
            (Some(version), Some(manifest))
        });
        releases.push(ResolvedRelease {
            package: owner.package,
            version,
            manifest,
        });
    }
    releases
}

/// The nearest `node_modules/<package>/package.json` above `start` whose
/// `version` field is a string, as `(version, manifest)`. A manifest with no
/// version string is skipped exactly as [`resolved_solid_version`] skips one.
fn nearest_package_version(start: &Path, package: &str) -> Option<(String, PathBuf)> {
    let start = if start.is_dir() {
        start
    } else {
        start.parent()?
    };
    start.ancestors().find_map(|directory| {
        let manifest = directory
            .join("node_modules")
            .join(package)
            .join("package.json");
        let encoded = std::fs::read_to_string(&manifest).ok()?;
        let version = serde_json::from_str::<serde_json::Value>(&encoded)
            .ok()?
            .get("version")?
            .as_str()?
            .to_owned();
        Some((version, manifest))
    })
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

/// The Solid 2 language: the vocabulary whose variant key is `None`,
/// [`solid_dialect::Solid2::CONSERVATIVE`], so the plain `solid-v2` id names
/// the same answers whichever release is audited. The audited triple and a
/// defaulted project reach theirs through the review and [`default_dialect`];
/// today both are this one.
#[cfg(feature = "dialect-v2")]
static SOLID_V2: Dialect = SOLID_V2_LANGUAGE;

/// The rc.9 triple's dialect: the [`RELEASE_VARIANTS`] entry whose vocabulary
/// is [`solid_dialect::Solid2::RC9`]. Named for tests; detection reaches it
/// through the installation review like any other variant.
#[cfg(all(test, feature = "dialect-v2"))]
static SOLID_V2_RC9: std::sync::LazyLock<&'static Dialect> =
    std::sync::LazyLock::new(|| for_vocabulary(&SOLID_V2, Some(&solid_dialect::Solid2::RC9)));

#[cfg(feature = "dialect-v2")]
const SOLID_V2_LANGUAGE: Dialect = Dialect {
    id: "solid-v2",
    compiler_facts_identity: solid_v2_compiler::COMPILER_FACTS_IDENTITY,
    vocabulary: &solid_dialect::Solid2::CONSERVATIVE,
    rule_count: solid_v2_rules::Rule::ALL.len(),
    compiler: || Box::new(solid_v2_compiler::NativeCompilerFacts),
    solve_measured: solid_v2_rules::solve_measured,
    docs_url: solid_v2_rules::docs_url,
    has_rule: |name| {
        solid_v2_rules::Rule::ALL.into_iter().any(|rule| {
            let metadata = rule.metadata();
            metadata.name == name && metadata.configurable
        })
    },
    has_code: |code| {
        solid_v2_rules::Rule::ALL
            .into_iter()
            .any(|rule| rule.metadata().code == code)
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
mod tests;
