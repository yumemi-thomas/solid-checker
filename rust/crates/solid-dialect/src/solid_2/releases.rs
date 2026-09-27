//! Which Solid 2 releases this vocabulary was read on, and what it answers for
//! each installation of them.
//!
//! Detection selects the Solid 2 *language* from the installed `solid-js`
//! major. This is the second question, and it is not about `solid-js` alone.
//! Solid 2 ships as three archives, and each release-dependent answer belongs
//! to the one that declares it:
//!
//! | answer | owner | resolved from |
//! | --- | --- | --- |
//! | B1: is a store root's own property `readonly` to TypeScript? | `@solidjs/signals` (`Store<T>`, `dist/types/store/store.d.ts:4`) | the installed `solid-js`, which re-exports `createStore` |
//! | B4: is `until` an export? | `solid-js` (the root re-export, from rc.5) *and* `@solidjs/signals` (the declaration, from rc.5) | as above |
//! | B2: does `dynamic(source, { static: true })` select a different runtime? | `@solidjs/web` (`if (options?.static)`, rc.9 only) | the project |
//! | B3: does `omit`'s lone function argument run as a predicate? | `@solidjs/signals` (rc.9 only) | the installed `solid-js` |
//! | N3: does the dev store-setter guard reject a root owner? | `@solidjs/signals` (`devGuardStoreSetterWrite`, rc.9 only) | the installed `solid-js` |
//! | N4: does `flush` throw `FLUSH_IN_ACTION` inside an action step? | `@solidjs/signals` (`dist/dev-shared.js`, from rc.8) | the installed `solid-js` |
//! | N5: does an optimistic-store setter meet the owned-scope write guard? | `@solidjs/signals` (`devGuardStoreSetterWrite`, from rc.1) | the installed `solid-js` |
//!
//! Every `solid-js@2.0.0-rc.N` depends on `@solidjs/signals: ^2.0.0-rc.N`, a
//! range, so a fresh install of the audited `solid-js@2.0.0-rc.3` resolves
//! `@solidjs/signals@2.0.0-rc.9` today
//! (`2026-09-26-solid-2-rc1-rc8-release-review.md` § 5). Reading only
//! `solid-js` answered B1 as rc.3 does on bytes that declare rc.9's
//! `Store<T> = T`, and the checker certified a store root write the runtime
//! drops. So each answer is taken from the resolved release of its owner, and
//! the variant is built from the resolved (`solid-js`, `@solidjs/signals`,
//! `@solidjs/web`) triple.
//!
//! ## Releases, per package (the two 2026-09-26 reviews)
//!
//! | release | B1 store root | B2 `dynamic` options | B3 `omit` predicate | B4 `until` | N3 store setter under a root | N4 `FLUSH_IN_ACTION` | open gaps |
//! | --- | --- | --- | --- | --- | --- | --- | --- |
//! | rc.0-rc.3 | `Readonly` | ignored | absent | absent | exempt (rc.0: see `StoreSetterRootGuard`) | absent | none: audited (rc.3), or equal to rc.0/rc.3 on every premise the dialect cites but N5 (rc.0, rc.1, rc.2) |
//! | rc.4 | `Readonly` | ignored | absent | absent | exempt | absent | `solid-js`: `registerPatch`, `registerRowOps`, `registerSlotPatch`; `@solidjs/web`: `installListDriver`, `driveList` (callback-taking, neither modelled nor excluded) |
//! | rc.5, rc.6 | `Readonly` | ignored | absent | present | exempt | absent | as rc.4 |
//! | rc.7 | `Mutable` | ignored (`DynamicOptions` is `deferStream` only, and no bundle reads it on the client) | absent | present | exempt | absent | `@solidjs/signals`: no negative row |
//! | rc.8 | `Mutable` | as rc.7 | absent | present | exempt | present | as rc.7 |
//! | rc.9 | `Mutable` | `static` selects `staticDynamic(untrack(source))` | present | present | guarded | present | `@solidjs/signals`: negative rows for five creates answers only; `solid-js`: re-exports its declarations do not declare |
//! | anything else (rc.10+, betas, `2.0.0`, an inexact spelling) | `Readonly` (see below) | not modelled | absent | not modelled | exempt (see below) | not modelled | the release is named as not compared |
//! | not resolved | `Readonly` (see below) | as rc.3 (nothing can import `dynamic`) | absent | not modelled | exempt (see below) | not modelled | named for `@solidjs/signals`; none for `@solidjs/web` |
//!
//! N5 is `guarded` on every row but rc.0's, whose optimistic-store setter
//! meets no owned-scope guard at all (`OptimisticStoreSetterGuard`); an unread
//! or unresolved signals keeps the audited `guarded`.
//!
//! `2.0.0-experimental.x` of `solid-js` is refused, not analyzed.
//!
//! An installation whose three packages sit at different rows is judged per
//! answer, each from its owner, and gets one more gap saying no review read the
//! combination. rc.0-rc.3 count as one row for that purpose. An `@solidjs/web`
//! that does not resolve at all adds no gap: nothing then imports `dynamic`,
//! so the one answer it owns is never asked.
//!
//! ## The default for a release nobody read
//!
//! An `@solidjs/signals` that is unknown or does not resolve keeps the
//! **`Readonly`** answer, so SC2003 stays silent on a store root write, and the
//! notice says root writes are unchecked. The other answer would report the
//! write, and that is not safe: every signals release before rc.7 declares
//! `Store<T> = Readonly<T>`, and `tsc` reports TS2540 on exactly that write
//! (the rc.1-rc.8 review § 2.3, rc.0-rc.6), so on an unknown *older* release
//! (a beta, a hand-built tarball) the finding would duplicate TypeScript. On
//! an unknown newer release silence misses a write the runtime drops, but the
//! notice already keeps the project from certifying, so the miss is visible.
//! Only one of the two defaults can break the absolute rule, so the default is
//! the other one.
//!
//! N3 defaults the same way, for the same kind of reason: an unknown signals
//! keeps the root exemption, so a store setter directly in a `createRoot` body,
//! or in a component body (a root in dev), is not reported there. Reporting it on a release that exempts roots would be
//! a violation the runtime does not raise; silence is a miss the notice already
//! makes visible.
//!
//! `until`, the `dynamic` option forms and the `FLUSH_IN_ACTION` throw are
//! simply not modelled on an unknown owner: `until` is not a vocabulary name,
//! an option-bearing `dynamic` call is the form that states nothing, and
//! `flush` in an action step is not claimed to throw.
//!
//! Matching is exact on the trimmed string. A range (`^2.0.0-rc.3`) or build
//! metadata (`2.0.0-rc.3+local`) is not the release that was read.

use std::sync::LazyLock;

use super::Solid2;
use crate::{
    Dialect, InstallationGap, InstallationReview, InstalledRelease, Primitive, RefusedRelease,
    ReleaseOwner,
};

const SOLID_JS: &str = "solid-js";
const SIGNALS: &str = "@solidjs/signals";
const WEB: &str = "@solidjs/web";

/// The three owners, in the order [`Solid2`]'s answers are read from them.
pub(super) const OWNERS: &[ReleaseOwner] = &[
    ReleaseOwner {
        package: SOLID_JS,
        resolved_from: None,
    },
    // `solid-js` re-exports the reactive core, so the declarations a
    // project's `import { createStore } from "solid-js"` sees are the ones the
    // installed `solid-js` resolves -- not whatever `@solidjs/signals` the
    // project root would.
    ReleaseOwner {
        package: SIGNALS,
        resolved_from: Some(SOLID_JS),
    },
    // `@solidjs/web` does not depend on signals, and a project imports it
    // directly.
    ReleaseOwner {
        package: WEB,
        resolved_from: None,
    },
];

/// The audited triple, what SC9014 tells a user to pin.
pub(super) const AUDITED_INSTALLATION: &[(&str, &str)] = &[
    (SOLID_JS, "2.0.0-rc.3"),
    (SIGNALS, "2.0.0-rc.3"),
    (WEB, "2.0.0-rc.3"),
];

/// Whether a `createStore` root's own properties are declared `readonly`.
///
/// Answered from the resolved `@solidjs/signals`: rc.0-rc.6 declare
/// `Store<T> = Readonly<T>` and rc.7-rc.9 `Store<T> = T`
/// (`dist/types/store/store.d.ts:4` in each, the rc.1-rc.8 review § 3.1).
/// `tsc --noEmit` over the real published typings, `strict`, reports
/// `TS2540: Cannot assign to 'name' because it is a read-only property` on
/// `profile.name = "Grace"` through rc.6 and nothing from rc.7. The runtime
/// drops the write outside a setter on every release (probe H, dev and prod),
/// so under `Mutable` the write is this checker's and under `Readonly` it is
/// TypeScript's.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum StoreRootTyping {
    /// `Store<T> = Readonly<T>`: a root property write is TS2540. Also the
    /// answer for a signals release nobody read (module docs).
    #[default]
    Readonly,
    /// `Store<T> = T`: a root property write type-checks.
    Mutable,
}

/// Whether the resolved `@solidjs/signals`'s dev store-setter guard exempts a
/// root owner (N3 of the rc.9 review).
///
/// Read from each release's `dist/dev.js` (`dist/dev-shared.js` from rc.8)
/// under the rc.1-rc.8 and rc.9 reviews' unpacked tarballs, and probed on each
/// published triple's dev client build with a store setter directly in a
/// `createRoot` body:
///
/// | signals | guard | `createStore` setter (plain and derived) | `createOptimisticStore` setter |
/// | --- | --- | --- | --- |
/// | rc.0 | none at the setter entry; a `createStore` draft write reaches `setSignal`'s guard through `notifyStoreProperty` | throws `REACTIVE_WRITE_IN_OWNED_SCOPE` | legal |
/// | rc.1-rc.8 | `devGuardStoreSetterWrite`: `if (context && !context._root && …)` (rc.3 `dist/dev.js:4111`, rc.8 `dist/dev-shared.js:4857`) | legal | legal |
/// | rc.9 | `if (context && …)` (`dist/dev-shared.js:5878`, citing #3500: "Roots are NOT exempt") | throws | throws |
///
/// So rc.9 is the first release that removed the exemption. rc.0 had none to
/// remove, but its two store setters disagree and a write carries only "a
/// store setter", so rc.0 keeps [`Self::Exempt`], the answer that claims
/// nothing. A signal setter there throws on every release: `setSignal` never
/// exempted roots. Prod builds carry neither guard.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum StoreSetterRootGuard {
    /// A store setter under a root owner is not reported: legal on rc.1-rc.8,
    /// and the answer for rc.0 and for a signals release nobody read, the one
    /// that claims no write ([`Dialect::store_setter_guard_exempts_roots`]).
    #[default]
    Exempt,
    /// rc.9: every store setter's guard rejects a root, as `setSignal`'s does.
    Guarded,
}

/// Whether the setter `createOptimisticStore` returns meets the dev owned-scope
/// write guard at all, on the resolved `@solidjs/signals` (N5).
///
/// rc.0's `createOptimisticStore` returns `fn => storeSetter(wrappedStore, fn)`
/// over a store whose nodes are marked `STORE_OPTIMISTIC` and take the
/// optimistic engine's write path (`@solidjs/signals@2.0.0-rc.0`
/// `dist/dev.js:7181-7213`), which never reaches `setSignal`'s guard
/// (`:3154-3172`), and rc.0 has no setter-entry guard; rc.1 adds
/// `devGuardStoreSetterWrite` at the setter entry (`storeSetterNext`,
/// `dist/dev.js:6619-6620`, guard at `:3264-3280`) for every store setter.
/// Probed on every published rc.0-rc.9 dev client build:
///
/// | signals | optimistic-store setter in a memo compute | in an effect compute | `createStore` setter in a memo compute |
/// | --- | --- | --- | --- |
/// | rc.0 | legal | legal | throws `REACTIVE_WRITE_IN_OWNED_SCOPE` |
/// | rc.1-rc.9 | throws | throws | throws |
///
/// Under a root rc.0 is already silent ([`StoreSetterRootGuard::Exempt`]);
/// this answer is what the owned scopes proper need. Prod builds carry no
/// guard on any release.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum OptimisticStoreSetterGuard {
    /// rc.1-rc.9, and the answer for a signals release nobody read or that does
    /// not resolve: the audited release's, under its `SC9014` notice.
    #[default]
    Guarded,
    /// rc.0: the optimistic store's writes bypass every owned-scope guard, so
    /// an optimistic-store setter is legal in any owned scope.
    Unguarded,
}

/// What `dynamic`'s second argument does on the resolved `@solidjs/web`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum DynamicOptions {
    /// rc.0-rc.8. rc.0-rc.6 declare `dynamic(source)` with one parameter, and
    /// rc.7/rc.8 `dynamic(source, _options?: DynamicOptions)` with
    /// `DynamicOptions { deferStream?: boolean }`, which the client bundle never
    /// reads (`dist/web.dev.js:2042` on rc.7, `:2074` on rc.8). No bundle before
    /// rc.9 contains `options?.static`, so every call is the default form,
    /// whatever its options say.
    #[default]
    Ignored,
    /// rc.9: `if (options?.static)` opens every build (`dist/web.dev.js:2199`),
    /// so a literal `static: true` is `staticDynamic(untrack(source))`.
    StaticForm,
    /// A web release nobody read: an option-bearing call is the form that
    /// states nothing.
    Unread,
}

impl Solid2 {
    /// The vocabulary as audited on `2.0.0-rc.3` (and read on `rc.0`): every
    /// answer the rc.3 triple gives.
    pub const AUDITED: Self = Self::from_index(0);

    /// The vocabulary for the `2.0.0-rc.9` triple: a mutable store root (B1),
    /// `dynamic`'s static form (B2), `omit`'s predicate form (B3), `until`
    /// (B4), a store-setter guard that no longer exempts roots (N3), and the
    /// `FLUSH_IN_ACTION` throw (N4).
    pub const RC9: Self = Self {
        store_root: StoreRootTyping::Mutable,
        omit_predicate_form: true,
        until: true,
        dynamic_options: DynamicOptions::StaticForm,
        store_setter_roots: StoreSetterRootGuard::Guarded,
        flush_in_action: true,
        optimistic_store_setter: OptimisticStoreSetterGuard::Guarded,
    };

    /// How many distinct vocabularies the answers above combine into: every
    /// combination is reachable, because the three owners install
    /// independently.
    const VARIANT_COUNT: usize = 2 * 2 * 2 * 3 * 2 * 2 * 2;

    /// The vocabulary at one mixed-radix index, the audited one at `0`. A new
    /// release-dependent answer adds one digit here and in
    /// [`Solid2::index`], and one token in [`variant_key`].
    const fn from_index(index: usize) -> Self {
        Self {
            store_root: if index % 2 == 1 {
                StoreRootTyping::Mutable
            } else {
                StoreRootTyping::Readonly
            },
            omit_predicate_form: (index / 2) % 2 == 1,
            until: (index / 4) % 2 == 1,
            dynamic_options: match (index / 8) % 3 {
                0 => DynamicOptions::Ignored,
                1 => DynamicOptions::StaticForm,
                _ => DynamicOptions::Unread,
            },
            store_setter_roots: if (index / 24) % 2 == 1 {
                StoreSetterRootGuard::Guarded
            } else {
                StoreSetterRootGuard::Exempt
            },
            flush_in_action: (index / 48) % 2 == 1,
            optimistic_store_setter: if (index / 96) % 2 == 1 {
                OptimisticStoreSetterGuard::Unguarded
            } else {
                OptimisticStoreSetterGuard::Guarded
            },
        }
    }

    const fn index(self) -> usize {
        let store = match self.store_root {
            StoreRootTyping::Readonly => 0,
            StoreRootTyping::Mutable => 1,
        };
        let omit = if self.omit_predicate_form { 1 } else { 0 };
        let until = if self.until { 1 } else { 0 };
        let dynamic = match self.dynamic_options {
            DynamicOptions::Ignored => 0,
            DynamicOptions::StaticForm => 1,
            DynamicOptions::Unread => 2,
        };
        let store_setter = match self.store_setter_roots {
            StoreSetterRootGuard::Exempt => 0,
            StoreSetterRootGuard::Guarded => 1,
        };
        let flush = if self.flush_in_action { 1 } else { 0 };
        let optimistic = match self.optimistic_store_setter {
            OptimisticStoreSetterGuard::Guarded => 0,
            OptimisticStoreSetterGuard::Unguarded => 1,
        };
        store
            + 2 * omit
            + 4 * until
            + 8 * dynamic
            + 24 * store_setter
            + 48 * flush
            + 96 * optimistic
    }

    /// The one `'static` value per vocabulary, which is what an analysis holds.
    pub(super) fn interned(self) -> &'static Self {
        &VARIANTS[self.index()]
    }

    pub(super) fn key(self) -> Option<&'static str> {
        KEYS[self.index()].as_deref()
    }

    /// Whether this installation exports `primitive` at all. Only `until`
    /// depends on the release; every other `TABLE` row is exported by
    /// every release the reviews read.
    pub(super) fn exports(self, primitive: Primitive) -> bool {
        primitive != Primitive::Until || self.until
    }
}

static VARIANTS: [Solid2; Solid2::VARIANT_COUNT] = {
    let mut variants = [Solid2::AUDITED; Solid2::VARIANT_COUNT];
    let mut index = 0;
    while index < Solid2::VARIANT_COUNT {
        variants[index] = Solid2::from_index(index);
        index += 1;
    }
    variants
};

/// The key naming how a variant differs from the audited vocabulary, one
/// token per answer it moves. `None` for the audited vocabulary itself.
fn variant_key(vocabulary: Solid2) -> Option<String> {
    let mut tokens = Vec::new();
    if vocabulary.store_root == StoreRootTyping::Mutable {
        tokens.push("store-root-mutable");
    }
    if vocabulary.omit_predicate_form {
        tokens.push("omit-predicate");
    }
    if vocabulary.until {
        tokens.push("until");
    }
    if vocabulary.flush_in_action {
        tokens.push("flush-in-action");
    }
    match vocabulary.dynamic_options {
        DynamicOptions::Ignored => {}
        DynamicOptions::StaticForm => tokens.push("dynamic-static"),
        DynamicOptions::Unread => tokens.push("dynamic-options-unread"),
    }
    if vocabulary.store_setter_roots == StoreSetterRootGuard::Guarded {
        tokens.push("store-setter-guards-roots");
    }
    if vocabulary.optimistic_store_setter == OptimisticStoreSetterGuard::Unguarded {
        tokens.push("optimistic-store-setter-unguarded");
    }
    (!tokens.is_empty()).then(|| tokens.join("+"))
}

static KEYS: LazyLock<Vec<Option<String>>> = LazyLock::new(|| {
    VARIANTS
        .iter()
        .map(|vocabulary| variant_key(*vocabulary))
        .collect()
});

/// Every variant but the audited vocabulary, for [`Dialect::variants`].
pub(super) static OTHER_VARIANTS: LazyLock<Vec<&'static dyn Dialect>> = LazyLock::new(|| {
    VARIANTS[1..]
        .iter()
        .map(|vocabulary| vocabulary as &'static dyn Dialect)
        .collect()
});

/// The review of the rc.9 triple.
const RC9_REVIEW: &str =
    "docs/package-contract-v2/audits/2026-09-26-solid-2-rc9-vocabulary-review.md";
/// The review of rc.1, rc.2 and rc.4-rc.8, and of mixed installs (§ 5).
const RC1_RC8_REVIEW: &str =
    "docs/package-contract-v2/audits/2026-09-26-solid-2-rc1-rc8-release-review.md";

/// The newest `2.0.0-rc.N` any review read.
const NEWEST_READ: u8 = 9;

/// A gap one reviewed release of one owner leaves open.
struct KnownGap {
    package: &'static str,
    /// The `2.0.0-rc.N` releases it applies to, inclusive.
    from: u8,
    through: u8,
    gap: &'static str,
    review: &'static str,
}

/// Every open gap a review measured, by owner and release. A gap leaves this
/// list when the dialect models it.
const KNOWN_GAPS: &[KnownGap] = &[
    // The rc.1-rc.8 review § 4.2: `<j4>/types/index.d.ts:1` re-exports them
    // from signals, and rc.7 drops all three.
    KnownGap {
        package: SOLID_JS,
        from: 4,
        through: 6,
        gap: "re-exports registerPatch, registerRowOps and registerSlotPatch, which take \
              callbacks this vocabulary neither models nor excludes, so code inside one is not \
              classified",
        review: RC1_RC8_REVIEW,
    },
    KnownGap {
        package: WEB,
        from: 4,
        through: 6,
        gap: "exports installListDriver and driveList, which take callbacks this vocabulary \
              neither models nor excludes, so code inside one is not classified",
        review: RC1_RC8_REVIEW,
    },
    // The rc.1-rc.8 review § 0.2: rc.7 and rc.8 are "reviewed with one gap".
    KnownGap {
        package: SIGNALS,
        from: 7,
        through: 8,
        gap: "has no negative row granted for it, so certification closes fewer claim domains \
              than on the audited release",
        review: RC1_RC8_REVIEW,
    },
    // The rc.9 review § 4.
    KnownGap {
        package: SOLID_JS,
        from: 9,
        through: 9,
        gap: "re-exports createErrorBoundary, createLoadingBoundary, createRevealOrder, \
              sharedConfig and $DEVCOMP, which its own declarations no longer declare, so under \
              skipLibCheck those primitives are not resolved and their bodies are not analyzed \
              as such",
        review: RC9_REVIEW,
    },
    // 59643beb read five creates rows on rc.9's own bytes, and nothing else.
    KnownGap {
        package: SIGNALS,
        from: 9,
        through: 9,
        gap: "has negative rows granted only for the creates domain of getOwner, onCleanup, \
              untrack, runWithOwner and createRoot, so certification closes fewer claim domains \
              than on the audited release",
        review: RC9_REVIEW,
    },
];

/// The pre-beta Solid 2 experiment. Measured on `2.0.0-experimental.1`
/// (corvu's install, the rc.9 review's § 8).
static PRE_BETA_EXPERIMENT: RefusedRelease = RefusedRelease {
    line: "2.0.0-experimental.x",
    reason: "it is the pre-beta Solid 2 experiment, which runs @solidjs/signals 0.x rather than \
             the release-candidate runtime this vocabulary was read on: its createEffect takes an \
             error handler at argument 2 and its createMemo a seed value at argument 1, where the \
             vocabulary reads options, and its Suspense and ErrorBoundary are not the Loading and \
             Errored boundaries the rules recognise",
    review: RC9_REVIEW,
};

/// One owner's resolved release, as far as the reviews go.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Release<'a> {
    /// `2.0.0-rc.N`, exactly, with `N` a release some review read.
    Read(u8),
    /// A version string no review read, as spelled.
    Unread(&'a str),
    /// Nothing resolved.
    Unresolved,
}

impl<'a> Release<'a> {
    fn of(version: Option<&'a str>) -> Self {
        let Some(version) = version else {
            return Self::Unresolved;
        };
        version
            .trim()
            .strip_prefix("2.0.0-rc.")
            .filter(|number| {
                !number.is_empty()
                    && number.bytes().all(|byte| byte.is_ascii_digit())
                    && (*number == "0" || !number.starts_with('0'))
            })
            .and_then(|number| number.parse::<u8>().ok())
            .filter(|number| *number <= NEWEST_READ)
            .map_or(Self::Unread(version), Self::Read)
    }

    /// The row of the table this release reads as, for telling a mixed
    /// installation apart: rc.0-rc.3 are one row.
    fn row(self) -> Option<u8> {
        match self {
            Self::Read(number) => Some(number.max(3)),
            Self::Unread(_) | Self::Unresolved => None,
        }
    }

    fn spelled(self) -> String {
        match self {
            Self::Read(number) => format!("2.0.0-rc.{number}"),
            Self::Unread(version) => version.trim().to_owned(),
            Self::Unresolved => "(not resolved)".to_owned(),
        }
    }
}

fn is_pre_beta_experiment(version: &str) -> bool {
    version
        .trim()
        .strip_prefix("2.0.0-")
        .is_some_and(|prerelease| {
            prerelease == "experimental" || prerelease.starts_with("experimental.")
        })
}

/// The vocabulary for one resolved triple: each answer from its owner.
fn vocabulary_for(solid_js: Release<'_>, signals: Release<'_>, web: Release<'_>) -> Solid2 {
    Solid2 {
        // B1. An unread or unresolved signals keeps `Readonly` (module docs).
        store_root: match signals {
            Release::Read(number) if number >= 7 => StoreRootTyping::Mutable,
            Release::Read(_) | Release::Unread(_) | Release::Unresolved => {
                StoreRootTyping::Readonly
            }
        },
        // B4. `until` is imported from the `solid-js` root, which re-exports
        // it from rc.5 (`<j5>/types/index.d.ts:1`), and declared by signals
        // from rc.5 (`<s5>/dist/types/signals.d.ts:601`). Either side older,
        // or unread, and the name is not one the vocabulary knows: on rc.3's
        // `solid-js` the import is TS2305 even over signals rc.9.
        until: matches!(
            (solid_js, signals),
            (Release::Read(root), Release::Read(core)) if root >= 5 && core >= 5
        ),
        // B2. No `@solidjs/web` at all leaves nothing to import `dynamic`
        // from, so the answer is never asked; the audited one keeps the
        // vocabulary the language's own.
        dynamic_options: match web {
            Release::Read(NEWEST_READ) => DynamicOptions::StaticForm,
            Release::Read(_) | Release::Unresolved => DynamicOptions::Ignored,
            Release::Unread(_) => DynamicOptions::Unread,
        },
        // B3. `omit` is `@solidjs/signals`'s (`solid-js` re-exports it), and
        // only rc.9's runtime tests `typeof keys[0] === "function"`
        // (`dist/dev.js:4380`; the rc.1-rc.8 review § 3.3 finds no earlier
        // release that does). An unread or unresolved signals keeps `false`.
        omit_predicate_form: matches!(signals, Release::Read(NEWEST_READ)),
        // N3. `devGuardStoreSetterWrite` is signals' (`solid-js` re-exports
        // `createStore`), and rc.9 is the only release whose store setters all
        // reject a root (`StoreSetterRootGuard`). Every other answer, unread
        // and unresolved included, keeps the one that reports nothing.
        store_setter_roots: match signals {
            Release::Read(NEWEST_READ) => StoreSetterRootGuard::Guarded,
            Release::Read(_) | Release::Unread(_) | Release::Unresolved => {
                StoreSetterRootGuard::Exempt
            }
        },
        // N4. `flush` is `@solidjs/signals`'s (`solid-js` re-exports it), and
        // the `actionStepDepth` guard first ships in its rc.8
        // (`dist/dev-shared.js:1904-1913`; the rc.1-rc.8 review § 3, probe R:
        // rc.0-rc.7 resolve, rc.8 and rc.9 reject with `FLUSH_IN_ACTION` in
        // dev). An unread or unresolved signals keeps `false`: the throw is
        // not claimed on bytes nobody read.
        flush_in_action: matches!(signals, Release::Read(number) if number >= 8),
        // N5. The optimistic store's write path is signals', and only rc.0's
        // skips every owned-scope guard (`OptimisticStoreSetterGuard`). Every
        // other answer, unread and unresolved included, keeps the audited
        // release's guard.
        optimistic_store_setter: match signals {
            Release::Read(0) => OptimisticStoreSetterGuard::Unguarded,
            Release::Read(_) | Release::Unread(_) | Release::Unresolved => {
                OptimisticStoreSetterGuard::Guarded
            }
        },
    }
}

/// Every gap in what the reviews know about one resolved triple.
fn gaps_for(solid_js: Release<'_>, signals: Release<'_>, web: Release<'_>) -> Vec<InstallationGap> {
    let mut gaps = Vec::new();
    for (package, release) in [(SOLID_JS, solid_js), (SIGNALS, signals), (WEB, web)] {
        match release {
            Release::Read(number) => gaps.extend(
                KNOWN_GAPS
                    .iter()
                    .filter(|known| {
                        known.package == package && (known.from..=known.through).contains(&number)
                    })
                    .map(|known| InstallationGap {
                        gap: format!("{package}@{} {}", release.spelled(), known.gap),
                        review: Some(known.review),
                    }),
            ),
            Release::Unread(_) => gaps.push(InstallationGap {
                gap: format!(
                    "{package} {} has not been compared against this vocabulary, so {}",
                    release.spelled(),
                    unread_consequence(package)
                ),
                review: None,
            }),
            Release::Unresolved => {
                if let Some(consequence) = unresolved_consequence(package) {
                    gaps.push(InstallationGap {
                        gap: consequence.to_owned(),
                        review: None,
                    });
                }
            }
        }
    }
    let mut rows = [solid_js, signals, web]
        .into_iter()
        .filter_map(Release::row)
        .collect::<Vec<_>>();
    rows.sort_unstable();
    rows.dedup();
    if rows.len() > 1 {
        gaps.push(InstallationGap {
            gap: format!(
                "solid-js {}, @solidjs/signals {} and @solidjs/web {} are not one reviewed \
                 release, and no review read this combination: each release-dependent answer is \
                 taken from the package that declares it (the store typing, the store \
                 setter's root guard, the optimistic-store setter's guard and flush's \
                 action-step throw from @solidjs/signals, until \
                 from solid-js and @solidjs/signals together, dynamic's options from \
                 @solidjs/web)",
                solid_js.spelled(),
                signals.spelled(),
                web.spelled()
            ),
            review: Some(RC1_RC8_REVIEW),
        });
    }
    gaps
}

/// What the vocabulary cannot answer when an owner's release was not read.
fn unread_consequence(package: &str) -> &'static str {
    match package {
        SIGNALS => {
            "the store typing it declares is unknown: a write to a store root's own property \
             outside a setter is not reported, nor is a store setter called directly in a \
             createRoot or component body, or flush() in an action body, and until is not \
             modelled"
        }
        WEB => {
            "a dynamic call with options is not modelled, and every other export it declares is \
             read as 2.0.0-rc.3 declares it"
        }
        _ => {
            "until is not modelled, and every other export it declares is read as 2.0.0-rc.3 \
             declares it"
        }
    }
}

/// What the vocabulary cannot answer when an owner did not resolve at all.
/// `None` for `@solidjs/web`: without it nothing imports `dynamic`, the one
/// answer it owns.
fn unresolved_consequence(package: &str) -> Option<&'static str> {
    match package {
        SIGNALS => Some(
            "@solidjs/signals does not resolve from the installed solid-js, so the store typing \
             it declares is unknown: a write to a store root's own property outside a setter is \
             not reported, nor is a store setter called directly in a createRoot or component \
             body, or flush() in an action body, and until is not modelled",
        ),
        WEB => None,
        _ => Some(
            "solid-js does not resolve, so no answer this vocabulary gives by release is known",
        ),
    }
}

/// The classification in the module table.
pub(super) fn review(installed: &[InstalledRelease<'_>]) -> InstallationReview {
    let version_of = |package: &str| {
        installed
            .iter()
            .find(|release| release.package == package)
            .and_then(|release| release.version)
    };
    if version_of(SOLID_JS).is_some_and(is_pre_beta_experiment) {
        return InstallationReview::Refused(&PRE_BETA_EXPERIMENT);
    }
    let solid_js = Release::of(version_of(SOLID_JS));
    let signals = Release::of(version_of(SIGNALS));
    let web = Release::of(version_of(WEB));
    InstallationReview::Analyzed {
        vocabulary: Some(vocabulary_for(solid_js, signals, web).interned()),
        gaps: gaps_for(solid_js, signals, web),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triple<'a>(
        solid_js: Option<&'a str>,
        signals: Option<&'a str>,
        web: Option<&'a str>,
    ) -> [InstalledRelease<'a>; 3] {
        [
            InstalledRelease {
                package: SOLID_JS,
                version: solid_js,
            },
            InstalledRelease {
                package: SIGNALS,
                version: signals,
            },
            InstalledRelease {
                package: WEB,
                version: web,
            },
        ]
    }

    fn same(release: &str) -> [InstalledRelease<'_>; 3] {
        triple(Some(release), Some(release), Some(release))
    }

    /// The vocabulary and gaps an installation is analyzed with.
    fn analyzed(installed: &[InstalledRelease<'_>]) -> (Solid2, Vec<InstallationGap>) {
        match review(installed) {
            InstallationReview::Analyzed {
                vocabulary: Some(vocabulary),
                gaps,
            } => {
                let variant = VARIANTS
                    .iter()
                    .find(|candidate| {
                        std::ptr::addr_eq(
                            *candidate as *const Solid2,
                            vocabulary as *const dyn Dialect,
                        )
                    })
                    .copied()
                    .expect("the vocabulary is one of the interned variants");
                (variant, gaps)
            }
            other => panic!("{installed:?} is analyzed: {other:?}"),
        }
    }

    /// The classification table, one same-release triple per row.
    #[test]
    fn every_reviewed_triple_answers_as_the_reviews_measured() {
        // (release, store root, omit predicate, until, dynamic options, store setter under a
        // root, flush in action, gap count)
        let rows = [
            (
                "2.0.0-rc.0",
                StoreRootTyping::Readonly,
                false,
                false,
                DynamicOptions::Ignored,
                StoreSetterRootGuard::Exempt,
                false,
                0,
            ),
            (
                "2.0.0-rc.1",
                StoreRootTyping::Readonly,
                false,
                false,
                DynamicOptions::Ignored,
                StoreSetterRootGuard::Exempt,
                false,
                0,
            ),
            (
                "2.0.0-rc.2",
                StoreRootTyping::Readonly,
                false,
                false,
                DynamicOptions::Ignored,
                StoreSetterRootGuard::Exempt,
                false,
                0,
            ),
            (
                "2.0.0-rc.3",
                StoreRootTyping::Readonly,
                false,
                false,
                DynamicOptions::Ignored,
                StoreSetterRootGuard::Exempt,
                false,
                0,
            ),
            (
                "2.0.0-rc.4",
                StoreRootTyping::Readonly,
                false,
                false,
                DynamicOptions::Ignored,
                StoreSetterRootGuard::Exempt,
                false,
                2,
            ),
            (
                "2.0.0-rc.5",
                StoreRootTyping::Readonly,
                false,
                true,
                DynamicOptions::Ignored,
                StoreSetterRootGuard::Exempt,
                false,
                2,
            ),
            (
                "2.0.0-rc.6",
                StoreRootTyping::Readonly,
                false,
                true,
                DynamicOptions::Ignored,
                StoreSetterRootGuard::Exempt,
                false,
                2,
            ),
            (
                "2.0.0-rc.7",
                StoreRootTyping::Mutable,
                false,
                true,
                DynamicOptions::Ignored,
                StoreSetterRootGuard::Exempt,
                false,
                1,
            ),
            (
                "2.0.0-rc.8",
                StoreRootTyping::Mutable,
                false,
                true,
                DynamicOptions::Ignored,
                StoreSetterRootGuard::Exempt,
                true,
                1,
            ),
            (
                "2.0.0-rc.9",
                StoreRootTyping::Mutable,
                true,
                true,
                DynamicOptions::StaticForm,
                StoreSetterRootGuard::Guarded,
                true,
                2,
            ),
        ];
        for (
            release,
            store_root,
            omit_predicate_form,
            until,
            dynamic_options,
            store_setter_roots,
            flush_in_action,
            gap_count,
        ) in rows
        {
            let (vocabulary, gaps) = analyzed(&same(release));
            assert_eq!(
                (
                    vocabulary.store_root,
                    vocabulary.omit_predicate_form,
                    vocabulary.until,
                    vocabulary.dynamic_options,
                    vocabulary.store_setter_roots,
                    vocabulary.flush_in_action
                ),
                (
                    store_root,
                    omit_predicate_form,
                    until,
                    dynamic_options,
                    store_setter_roots,
                    flush_in_action
                ),
                "{release}"
            );
            assert_eq!(gaps.len(), gap_count, "{release}: {gaps:?}");
        }
        assert_eq!(
            analyzed(&same("2.0.0-rc.3")).0.index(),
            Solid2::AUDITED.index()
        );
        assert_eq!(analyzed(&same("2.0.0-rc.9")).0.index(), Solid2::RC9.index());
        // rc.4-rc.6 name all five patch-channel exports between them.
        let names = analyzed(&same("2.0.0-rc.5"))
            .1
            .iter()
            .map(|gap| gap.gap.clone())
            .collect::<Vec<_>>()
            .join(" ");
        for export in [
            "registerPatch",
            "registerRowOps",
            "registerSlotPatch",
            "installListDriver",
            "driveList",
        ] {
            assert!(names.contains(export), "{export}: {names}");
        }
    }

    /// Defect 1 of the rc.1-rc.8 review § 5: a fresh install of the audited
    /// `solid-js` resolves signals rc.9, whose store root is mutable.
    #[test]
    fn the_store_typing_follows_the_resolved_signals() {
        let fresh = triple(Some("2.0.0-rc.3"), Some("2.0.0-rc.9"), Some("2.0.0-rc.3"));
        let (vocabulary, gaps) = analyzed(&fresh);
        assert_eq!(vocabulary.store_root, StoreRootTyping::Mutable);
        // `omit` is signals' too, so its predicate form comes with rc.9, and
        // so is the store setter's guard, which no longer exempts roots.
        assert!(vocabulary.omit_predicate_form);
        assert_eq!(vocabulary.store_setter_roots, StoreSetterRootGuard::Guarded);
        // `flush` is signals' as well, so its action-step throw comes along.
        assert!(vocabulary.flush_in_action);
        // rc.3's root does not re-export until (TS2305 on that tree), and
        // rc.3's web has no static option.
        assert!(!vocabulary.until);
        assert_eq!(vocabulary.dynamic_options, DynamicOptions::Ignored);
        assert!(
            gaps.iter()
                .any(|gap| gap.gap.contains("not one reviewed release")),
            "{gaps:?}"
        );
        // And the other way: rc.9's solid-js over an older signals.
        let (vocabulary, _) = analyzed(&triple(
            Some("2.0.0-rc.9"),
            Some("2.0.0-rc.6"),
            Some("2.0.0-rc.9"),
        ));
        assert_eq!(vocabulary.store_root, StoreRootTyping::Readonly);
        assert!(vocabulary.until);
        assert_eq!(vocabulary.dynamic_options, DynamicOptions::StaticForm);
        assert_eq!(vocabulary.store_setter_roots, StoreSetterRootGuard::Exempt);
        assert!(!vocabulary.flush_in_action);
    }

    /// N4: the `FLUSH_IN_ACTION` throw is signals' alone, from rc.8, and is
    /// never claimed on a signals release nobody read.
    #[test]
    fn the_flush_throw_follows_the_resolved_signals() {
        for (signals, throws) in [
            (Some("2.0.0-rc.3"), false),
            (Some("2.0.0-rc.7"), false),
            (Some("2.0.0-rc.8"), true),
            (Some("2.0.0-rc.9"), true),
            (Some("2.0.0-rc.10"), false),
            (Some("2.0.0-beta.2"), false),
            (None, false),
        ] {
            let (vocabulary, _) =
                analyzed(&triple(Some("2.0.0-rc.3"), signals, Some("2.0.0-rc.3")));
            assert_eq!(vocabulary.flush_in_action, throws, "{signals:?}");
            assert_eq!(
                vocabulary.throws_inside_action_step(Primitive::Flush),
                throws,
                "{signals:?}"
            );
            // The stepping is the same on every release; only the throw moves.
            assert!(vocabulary.callback_runs_as_action_steps(Primitive::Action, 0));
            assert!(!vocabulary.callback_runs_as_action_steps(Primitive::Action, 1));
            assert!(!vocabulary.throws_inside_action_step(Primitive::Untrack));
        }
    }

    /// N5: only rc.0's optimistic-store setter meets no owned-scope guard, and
    /// the answer is signals' alone; an unread or unresolved signals keeps the
    /// audited guard. rc.0's `createStore` answer (N3) is unchanged.
    #[test]
    fn the_optimistic_store_setter_guard_follows_the_resolved_signals() {
        for (signals, guarded) in [
            (Some("2.0.0-rc.0"), false),
            (Some("2.0.0-rc.1"), true),
            (Some("2.0.0-rc.3"), true),
            (Some("2.0.0-rc.9"), true),
            (Some("2.0.0-rc.10"), true),
            (Some("2.0.0-beta.2"), true),
            (None, true),
        ] {
            let (vocabulary, _) =
                analyzed(&triple(Some("2.0.0-rc.3"), signals, Some("2.0.0-rc.3")));
            assert_eq!(
                vocabulary.optimistic_store_setter_guarded(),
                guarded,
                "{signals:?}"
            );
        }
        let (rc0, _) = analyzed(&same("2.0.0-rc.0"));
        assert!(!rc0.optimistic_store_setter_guarded());
        assert!(rc0.store_setter_guard_exempts_roots());
        assert_eq!(rc0.store_setter_roots, StoreSetterRootGuard::Exempt);
        assert_eq!(rc0.key(), Some("optimistic-store-setter-unguarded"));
        assert!(Solid2::AUDITED.optimistic_store_setter_guarded());
        assert!(Solid2::RC9.optimistic_store_setter_guarded());
        // Rows rc.1-rc.3 still share the audited vocabulary.
        for release in ["2.0.0-rc.1", "2.0.0-rc.2", "2.0.0-rc.3"] {
            assert_eq!(
                analyzed(&same(release)).0.index(),
                Solid2::AUDITED.index(),
                "{release}"
            );
        }
    }

    /// Defect 2: B2 and B4 answer only where the owner has the feature.
    #[test]
    fn until_and_the_static_form_need_their_owners() {
        for (solid_js, signals, until) in [
            ("2.0.0-rc.4", "2.0.0-rc.9", false),
            ("2.0.0-rc.5", "2.0.0-rc.4", false),
            ("2.0.0-rc.5", "2.0.0-rc.5", true),
            ("2.0.0-rc.9", "2.0.0-rc.10", false),
        ] {
            let (vocabulary, _) = analyzed(&triple(Some(solid_js), Some(signals), None));
            assert_eq!(vocabulary.until, until, "{solid_js} over {signals}");
        }
        for (web, options) in [
            ("2.0.0-rc.3", DynamicOptions::Ignored),
            ("2.0.0-rc.7", DynamicOptions::Ignored),
            ("2.0.0-rc.8", DynamicOptions::Ignored),
            ("2.0.0-rc.9", DynamicOptions::StaticForm),
            ("2.0.0-rc.10", DynamicOptions::Unread),
        ] {
            let (vocabulary, _) =
                analyzed(&triple(Some("2.0.0-rc.9"), Some("2.0.0-rc.9"), Some(web)));
            assert_eq!(vocabulary.dynamic_options, options, "{web}");
        }
    }

    /// The conservative answers, each with a gap naming it.
    #[test]
    fn an_unknown_or_missing_owner_takes_the_conservative_answers() {
        for signals in [
            None,
            Some("2.0.0-rc.10"),
            Some("2.0.0-beta.2"),
            Some("^2.0.0-rc.3"),
        ] {
            let (vocabulary, gaps) =
                analyzed(&triple(Some("2.0.0-rc.9"), signals, Some("2.0.0-rc.9")));
            assert_eq!(
                vocabulary.store_root,
                StoreRootTyping::Readonly,
                "{signals:?}"
            );
            assert!(!vocabulary.until, "{signals:?}");
            assert_eq!(
                vocabulary.store_setter_roots,
                StoreSetterRootGuard::Exempt,
                "{signals:?}"
            );
            assert!(
                gaps.iter()
                    .any(|gap| gap.gap.contains("store root") && gap.review.is_none()),
                "{signals:?}: {gaps:?}"
            );
        }
        let (vocabulary, gaps) = analyzed(&triple(
            Some("2.0.0-rc.3"),
            Some("2.0.0-rc.3"),
            Some("2.0.1"),
        ));
        assert_eq!(vocabulary.dynamic_options, DynamicOptions::Unread);
        assert_eq!(gaps.len(), 1, "{gaps:?}");
        // A web that does not resolve is not asked about, so it adds no gap;
        // the rc.3 pair beside it is still audited.
        let (vocabulary, gaps) = analyzed(&triple(Some("2.0.0-rc.3"), Some("2.0.0-rc.3"), None));
        assert_eq!(vocabulary.index(), Solid2::AUDITED.index());
        assert!(gaps.is_empty(), "{gaps:?}");
        // An unknown solid-js keeps the audited answers it owns, with a gap.
        let (vocabulary, gaps) = analyzed(&triple(
            Some("2.0.0"),
            Some("2.0.0-rc.3"),
            Some("2.0.0-rc.3"),
        ));
        assert!(!vocabulary.until);
        assert_eq!(gaps.len(), 1, "{gaps:?}");
    }

    #[test]
    fn releases_read_only_their_exact_spelling() {
        for (version, release) in [
            ("2.0.0-rc.3", Release::Read(3)),
            (" 2.0.0-rc.3\n", Release::Read(3)),
            ("2.0.0-rc.0", Release::Read(0)),
            ("2.0.0-rc.9", Release::Read(9)),
            ("2.0.0-rc.10", Release::Unread("2.0.0-rc.10")),
            ("2.0.0-rc.03", Release::Unread("2.0.0-rc.03")),
            ("2.0.0-rc.3.1", Release::Unread("2.0.0-rc.3.1")),
            ("2.0.0-rc.3+local", Release::Unread("2.0.0-rc.3+local")),
            ("^2.0.0-rc.3", Release::Unread("^2.0.0-rc.3")),
            ("v2.0.0-rc.3", Release::Unread("v2.0.0-rc.3")),
            ("2.0.0", Release::Unread("2.0.0")),
            ("2.0.0-beta.19", Release::Unread("2.0.0-beta.19")),
        ] {
            assert_eq!(Release::of(Some(version)), release, "{version:?}");
        }
        assert_eq!(Release::of(None), Release::Unresolved);
    }

    #[test]
    fn the_pre_beta_experiment_is_refused_by_its_solid_js() {
        for refused in [
            "2.0.0-experimental.1",
            "2.0.0-experimental.0",
            "2.0.0-experimental",
        ] {
            assert!(
                matches!(
                    review(&same(refused)),
                    InstallationReview::Refused(refusal) if std::ptr::eq(refusal, &PRE_BETA_EXPERIMENT)
                ),
                "{refused}"
            );
        }
        assert!(matches!(
            review(&triple(
                Some("2.1.0-experimental.1"),
                Some("2.0.0-rc.3"),
                None
            )),
            InstallationReview::Analyzed { .. }
        ));
    }

    /// Every variant round-trips through its index, and no two share a key.
    #[test]
    fn every_variant_has_its_own_key() {
        let mut keys = std::collections::BTreeSet::new();
        for (index, vocabulary) in VARIANTS.iter().enumerate() {
            assert_eq!(vocabulary.index(), index);
            assert!(
                keys.insert(vocabulary.key()),
                "{index}: {:?}",
                vocabulary.key()
            );
            assert_eq!(vocabulary.key().is_none(), index == 0);
        }
        assert_eq!(OTHER_VARIANTS.len(), Solid2::VARIANT_COUNT - 1);
        assert_eq!(
            Solid2::RC9.key(),
            Some(
                "store-root-mutable+omit-predicate+until+flush-in-action+dynamic-static+store-setter-guards-roots"
            )
        );
    }

    /// What the engine asks, through the trait, follows the variant; the
    /// review does not depend on which variant is asked.
    #[test]
    fn the_trait_answers_from_the_variant() {
        assert!(Solid2::AUDITED.store_root_properties_are_readonly());
        assert!(!Solid2::RC9.store_root_properties_are_readonly());
        // The value the engine holds by name is the audited vocabulary.
        assert!(Solid2.store_root_properties_are_readonly());
        assert!(Solid2::default().store_root_properties_are_readonly());
        assert_eq!(Solid2::default().index(), Solid2::AUDITED.index());
        assert!(Solid2::RC9.store_setter_callback_enables_proxy_writes());
        assert!(Solid2::AUDITED.store_setter_guard_exempts_roots());
        assert!(!Solid2::RC9.store_setter_guard_exempts_roots());
        for release in 0..=8 {
            let release = format!("2.0.0-rc.{release}");
            assert!(
                analyzed(&same(&release))
                    .0
                    .store_setter_guard_exempts_roots(),
                "{release}"
            );
        }
        // Only `createRoot`'s own body is a created root.
        assert!(Solid2.callback_runs_in_created_root(Primitive::CreateRoot, 0));
        for (primitive, argument) in [
            (Primitive::CreateRoot, 1),
            (Primitive::RunWithOwner, 1),
            (Primitive::Untrack, 0),
            (Primitive::Flush, 0),
            (Primitive::CreateMemo, 0),
            (Primitive::Render, 0),
        ] {
            assert!(
                !Solid2.callback_runs_in_created_root(primitive, argument),
                "{primitive:?} {argument}"
            );
        }
        // `flush(fn)` creates no root: it keeps the caller's owner, as
        // `untrack` does, on every release (probed rc.0-rc.9).
        for vocabulary in [Solid2::AUDITED, Solid2::RC9] {
            assert!(vocabulary.callback_preserves_owner_write_context(Primitive::Flush));
            assert!(vocabulary.callback_preserves_owner_write_context(Primitive::Untrack));
            assert!(!vocabulary.callback_preserves_owner_write_context(Primitive::CreateRoot));
            // The dev component body runs under `createRoot(…, { transparent: true })`.
            assert!(vocabulary.component_body_runs_under_root());
        }
        for gated in super::super::RELEASE_GATED_NAMES {
            assert_eq!(Solid2::AUDITED.primitive(gated), None, "{gated}");
            assert!(Solid2::RC9.primitive(gated).is_some(), "{gated}");
        }
        let expected = Solid2::RC9.interned() as *const Solid2;
        for vocabulary in &VARIANTS {
            let InstallationReview::Analyzed {
                vocabulary: Some(chosen),
                ..
            } = vocabulary.review_installation(&same("2.0.0-rc.9"))
            else {
                panic!("rc.9 is analyzed");
            };
            assert!(std::ptr::addr_eq(chosen as *const dyn Dialect, expected));
        }
    }

    #[test]
    fn every_review_names_a_document_that_exists() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for document in [RC9_REVIEW, RC1_RC8_REVIEW, PRE_BETA_EXPERIMENT.review] {
            assert!(root.join(document).is_file(), "{document} is missing");
        }
        for known in KNOWN_GAPS {
            assert!(known.from <= known.through && known.through <= NEWEST_READ);
            assert!(OWNERS.iter().any(|owner| owner.package == known.package));
        }
    }
}
