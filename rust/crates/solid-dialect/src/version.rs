//! Which Solid language version a project targets, and what an installed release makes of it.

use super::*;

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

/// ADR 0334: one audited `@tanstack/solid-router` release and the bytes the
/// audit read: the router's own install and the `@tanstack/router-core` that
/// Node resolves from its real directory. Both digests are
/// `scripts/router-digest.mjs` values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuditedRouteRelease {
    pub version: &'static str,
    pub router_digest: &'static str,
    pub core_version: &'static str,
    pub core_digest: &'static str,
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
    /// Whether these exports run project code only while the application
    /// renders on a server, so an analysis whose rendering is declared
    /// client-only (`--rendering csr`) has nothing unclassified from them
    /// and the gap is not due for it. The rendering declaration is the
    /// user's premise, as it is for every server-rendering answer; an
    /// undeclared or server rendering keeps the gap.
    pub server_rendering_only: bool,
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
