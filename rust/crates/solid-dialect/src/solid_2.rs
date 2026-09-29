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
    AuditedArchive, AuditedCitation, Boundary, CallClaimDomain, CallbackOwner, CleanupRule,
    ContextRole, Dialect, DialectNegativeAuthority, Execution, HostTargetCondition,
    HostTargetScope, NegativeClaimRow, Primitive, ReactiveRole, ResultSlot, RowScope,
    TrackedCallbackTiming, TrackingRuntime, Version, lookup, reverse,
};

mod releases;

/// Solid 2.0, answering for one installation of its three archives.
///
/// Almost every answer is the same on every release this vocabulary was read
/// on, so the releases are carried as data rather than as more types: one field
/// per answer the reviewed releases disagree on, each decided by the resolved
/// release of the package that declares it (`releases.rs`, which builds the
/// value from the resolved `solid-js`, `@solidjs/signals` and `@solidjs/web`).
/// `Solid2` the value is the conservative vocabulary
/// ([`Solid2::CONSERVATIVE`]); [`Solid2::RC3`] and [`Solid2::RC9`] are those
/// triples', and the audited one is rc.9's. The audited vocabulary, the
/// conservative one variant ids are spelled against ([`Solid2::CONSERVATIVE`])
/// and the one a project with no `solid-js` is analyzed under
/// ([`Solid2::DEFAULTED`]) are three names, not one (`releases.rs`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Solid2 {
    /// B1, from `@solidjs/signals`.
    store_root: releases::StoreRootTyping,
    /// B3: whether this release's `omit` has the predicate form, a lone function
    /// argument the returned view invokes
    /// ([`Solid2::callback_runs_on_result_access`]).
    ///
    /// A runtime difference, not only a typing one:
    ///
    /// - `@solidjs/signals@2.0.0-rc.3`'s `omit(props, ...keys)` never invokes
    ///   a key. Every build only tests membership, `keys.includes(property)`
    ///   in the view's `get`/`has`/`keys` traps and in the non-`Proxy` copy,
    ///   and `new Set(keys)` above four keys: `dist/dev.js:9334-9369` (sha256
    ///   `cc68ed0f…1a79`), `dist/prod/store/utils.js:169-199`
    ///   (`ff62f6f1…acb9`) and `dist/node.cjs:7816` (`bc0e35d3…5c1c`), each
    ///   matching `benchmarks/package-contract-v2/phase0/rc3/solidjs-signals/files.json`.
    ///   A function passed there is a key that matches no property.
    /// - `2.0.0-rc.9` selects the predicate by
    ///   `keys.length === 1 && typeof keys[0] === "function"`
    ///   (`dist/dev.js:4380`) and calls it on every read of the view.
    ///
    /// TypeScript separates the two only where types exist (rc.3 rejects a
    /// function key with TS2345; rc.9 accepts it). In an untyped artifact
    /// nothing in the call does, so the answer comes from the release of the
    /// package that implements `omit`, the resolved `@solidjs/signals`
    /// (`solid-js` only re-exports it): `true` on rc.9 alone, since no
    /// runtime before it tests `typeof keys[0]` (the rc.1-rc.8 review § 3.3).
    /// A signals release nobody compared, or none resolved, keeps `false`,
    /// under its `SC9014` notice.
    omit_predicate_form: bool,
    /// B4: whether `until` is a name this installation exports, from
    /// `solid-js` and `@solidjs/signals` together. `false` answers `None` for
    /// the name everywhere the vocabulary is asked about it, so nothing reaches
    /// the `until` rows or SC2005.
    until: bool,
    /// N4 of the rc.9 review: whether `flush` throws `FLUSH_IN_ACTION` in dev
    /// while an action step is on the stack, from `@solidjs/signals`.
    ///
    /// `2.0.0-rc.8` added the guard (`dist/dev-shared.js:1904-1913`; rc.9
    /// `:2210-2219`): `if (actionStepDepth > 0) throw new Error("[FLUSH_IN_ACTION]
    /// …")` opens `flush`, before its `fn` argument is read, so `flush()` and
    /// `flush(fn)` both throw. `actionStepDepth` is raised only by `action`'s
    /// `step`, around `it.next(v)`/`it.throw(v)` (rc.8 `dist/dev.js:1682-1690`,
    /// rc.9 `:2016-2024`). The production and observe builds take the same
    /// branch and return `fn?.()` without draining (rc.9
    /// `dist/prod/core/scheduler.js:1182-1184`), so nothing throws there. No
    /// release before rc.8 has `actionStepDepth` at all (the rc.1-rc.8 review
    /// § 3, probe R). `true` only for a signals release some review read at
    /// rc.8 or later; an unread or unresolved one keeps `false`, which states
    /// nothing, under its `SC9014` notice.
    flush_in_action: bool,
    /// B2, from `@solidjs/web`.
    dynamic_options: releases::DynamicOptions,
    /// N3, from `@solidjs/signals`: whether its dev store-setter guard
    /// exempts a root owner ([`Dialect::store_setter_guard_exempts_roots`]).
    store_setter_roots: releases::StoreSetterRootGuard,
    /// N5, from `@solidjs/signals`: whether an optimistic-store setter meets
    /// the owned-scope write guard at all
    /// ([`Dialect::optimistic_store_setter_guarded`]).
    optimistic_store_setter: releases::OptimisticStoreSetterGuard,
}

/// The Solid 2 language's own vocabulary, [`Solid2::CONSERVATIVE`], spelled
/// like the unit struct it used to be so the value keeps its name everywhere
/// the engine and its tests hold it. It is not the audited release's: the
/// engine's tests pin the conservative answers, and a release-dependent
/// answer is tested on the variant that gives it.
/// A braced struct lives only in the type namespace, which leaves the value
/// namespace free for this constant.
#[allow(non_upper_case_globals)]
pub const Solid2: Solid2 = Solid2::CONSERVATIVE;

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
    // New in `2.0.0-rc.5`: `until<T>(fn: () => T, options?: UntilOptions):
    // Promise<Truthy<T>>` (`@solidjs/signals` `dist/types/signals.d.ts:601`
    // on rc.5, `:608` on rc.9), re-exported from the `solid-js` root
    // (`types/index.d.ts:1`) from the same release. rc.3's typings do not
    // export it (TS2305), so the row answers only where the installation has
    // it (`Solid2::until`); everywhere else the name is unknown.
    ("until", Primitive::Until),
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

/// The [`TABLE`] names only some installations export (`Solid2::exports`).
#[cfg(test)]
pub(crate) const RELEASE_GATED_NAMES: &[&str] = &["until"];

/// The exact published archives this dialect's negative rows were read
/// against. A test byte-checks every tuple, so a re-audit of different bytes
/// cannot leave a stale tuple behind, and the evidence differs by archive:
///
/// - The three `2.0.0-rc.3` tuples are the audited contract documents' own
///   `package` block, field for field (every bundled document under
///   `pkg/contracts/bundled/solid-v2/` naming the package must agree).
/// - `@solidjs/signals@2.0.0-rc.6` has no bundled document — its rows are all
///   implementation readings ([`RC6_SIGNALS_AUDIT`]) — so its tuple is checked
///   against the pinned `benchmarks/package-contract-v2/phase0/rc6/solidjs-signals/`
///   record instead: `package.json` carries the name and version and hashes to
///   `manifest_sha256`, and `files.json` pins that file at the same digest and
///   length. The integrity is the one 85 ecosystem install trees' `bun.lock`
///   record; no tarball was downloaded to re-derive it, so no test here can
///   check it. It is not load-bearing for soundness: the census gate compares
///   it against the certifier's own authenticated snapshot, so a wrong value
///   can only stop the rows from binding, never bind them to other bytes.
/// - `@solidjs/signals@2.0.0-rc.9` is checked the same way, against
///   `benchmarks/package-contract-v2/phase0/rc9/solidjs-signals/`. Its
///   integrity *was* re-derived: the 2026-09-26 audit ([`RC9_SIGNALS_AUDIT`])
///   downloaded the registry tarball, its SRI SHA-512 equals the tuple's (and
///   the solid-primitives lockfile's), and its extracted tree is byte-identical
///   to the install the audit read (`phase0/rc9/solidjs-signals/tarball.json`).
/// - `solid-js@2.0.0-rc.9` and `@solidjs/web@2.0.0-rc.9` are checked the same
///   way, against `phase0/rc9/solid-js/` and `phase0/rc9/solidjs-web/`. Both
///   tarballs were downloaded (2026-09-27, [`RC9_CORE_WEB_AUDIT`]); each SRI
///   SHA-512 equals the registry's and the solid-primitives lockfile's, and
///   each extracted tree is byte-identical to that lockfile's install.
///
/// `manifest_sha256` is the digest of the archive's own `package.json`, which
/// a caller re-derives from the authenticated snapshot (`sha256` of
/// `snapshot.read("package.json")`) rather than reading from a manifest a
/// resolver reported. All three rc.3 digests were confirmed against the
/// installed rc.3 trees, rc.6's against the installed rc.6 tree, and rc.9's
/// against both its installed tree and its registry tarball.
///
/// `solid-js@2.0.0-rc.3` is listed even though its audited document covers
/// only ten exports: the archive was read, and "read and found nothing to
/// deny about `createSignal`" has to be distinguishable from "never looked".
///
/// Listing an archive extends no row to it. Rows are archive-scoped
/// ([`NegativeClaimRow::version`]), so rc.6 answers only from the rows read on
/// rc.6's bytes, rc.9 only from the rows read on rc.9's, and rc.3's rows keep
/// answering for rc.3 alone.
const AUDITED_ARCHIVES: &[AuditedArchive] = &[
    AuditedArchive {
        name: "@solidjs/signals",
        version: "2.0.0-rc.3",
        integrity: "sha512-/yPhTf3xS1FRR4MX8kTYCd4MjsFxzwkO+KyOTfbu35lTEiaJ4Fxy+JL91XonDzt31GV1mYaZ9CGD2TQIzvXuNA==",
        manifest_sha256: "22d27a9ebdc7b4fbfc65b9857bbea96ea60d3617697fd628b42b6e1253ffdb76",
    },
    AuditedArchive {
        name: "@solidjs/signals",
        version: "2.0.0-rc.6",
        integrity: "sha512-lPqwZNLPq1Z9CBvgXkMvi1ZFr5OHUiFNz1X40+yehszDWEbJkneZx7BGKIe9eMT/AN1NSL+PMjOiMyZaqVB2xw==",
        manifest_sha256: "de11cde1dd28b678f380c865be674a1f1a18a198e399ad2f997fd83aef1c163c",
    },
    AuditedArchive {
        name: "@solidjs/signals",
        version: "2.0.0-rc.9",
        integrity: "sha512-o3pqiTgpH5NR2DstiKrt9s/6+0YOFtv+MfvLONwLsS247I+EWMMyTu9BkRcgd35UR5Pa1DM16lI1/5uaIMY6Gw==",
        manifest_sha256: "c612461c9264f2b3509ced91ea91019ed7b1ea0df0d64bba7f0f30c8d00bb1c6",
    },
    AuditedArchive {
        name: "@solidjs/web",
        version: "2.0.0-rc.3",
        integrity: "sha512-5ckKgOjem1pN5ADycOk6TjHmTtjbbN2fukqxo6RW3Oe3H7z0gaXWAdt8dLISto5/O4Nn8VxprFXFWpfy31+DUg==",
        manifest_sha256: "ee9b514b90b06b679d2376c5b5a993c0391aa66ec744e453ec3e534babd30e8e",
    },
    AuditedArchive {
        name: "@solidjs/web",
        version: "2.0.0-rc.9",
        integrity: "sha512-pfiWoLDnLc+QYWc7UyLqO+5QrPEf3oTiNmmRC+C+uM6AZ5VH0bZMNPtLM5rJ29LKPiTwQitKV843IQDf/oeyhQ==",
        manifest_sha256: "5de9244eb8121c5efe10f3976a06b09ca09cb99de4caf5fcf56dc040ce9773dc",
    },
    AuditedArchive {
        name: "solid-js",
        version: "2.0.0-rc.3",
        integrity: "sha512-pmW6bRoTvfp/rN4jN7JmLvSaoIpFt7wm0Hi3j508S/smuJqUbRg3dQEjOPTkAwHW+McYnXrMG7cJ4AMNpLevtQ==",
        manifest_sha256: "e703e7986516ac05ee91fdd64897c2d150aea948cb5bf77eae8673da5008ee4b",
    },
    AuditedArchive {
        name: "solid-js",
        version: "2.0.0-rc.9",
        integrity: "sha512-J/oHWnWqe7S0FeIEdIRKDvyyo+HY/TYKr2PrIB8VlePMWuErDg78QHqdsAV7f6HKa9qhWR/23eqzR/ZRV9ep0g==",
        manifest_sha256: "c8d6224bd4bd63ed38f0cec77eb4d30d3f78debec091338dc4e466a2618e11bc",
    },
];

/// The hand implementation census over the exact published rc.3 bytes of five
/// core `@solidjs/signals` primitives, and the source of every
/// [`AuditedCitation::Implementation`] below.
///
/// Signed off by delegation, 2026-09-04.
const RC3_CORE_PRIMITIVES_AUDIT: &str =
    "docs/package-contract-v2/audits/2026-09-04-solid-2-rc3-core-primitives-creates.md";

/// The hand implementation census of `runWithOwner`, `createContext` and
/// `useContext` over the same rc.3 bytes, in that audit's method (2026-09-23).
/// For the repository owner's review, as the first one was.
const RC3_OWNER_CONTEXT_AUDIT: &str =
    "docs/package-contract-v2/audits/2026-09-23-solid-2-rc3-owner-and-context-creates.md";

/// The re-audit of every `@solidjs/signals` row on the exact bytes of
/// `@solidjs/signals@2.0.0-rc.6`, the prerelease the ecosystem installs
/// (2026-09-25): 24 of the 25 rc.3 rows granted, `createOptimisticStore`
/// `reads` withheld. The source of every rc.6 [`AuditedCitation::Implementation`]
/// below; for the repository owner's review, as the two rc.3 audits were.
const RC6_SIGNALS_AUDIT: &str =
    "docs/package-contract-v2/audits/2026-09-25-solid-2-rc6-signals-negative-rows.md";

/// The reading of five `@solidjs/signals` `creates` rows — `getOwner`,
/// `onCleanup`, `createRoot`, `untrack`, `runWithOwner` — on the exact bytes of
/// `@solidjs/signals@2.0.0-rc.9` (2026-09-26), in every runtime build rc.9's
/// `exports` map can select: `dist/prod/**`, `dist/dev.js` with
/// `dist/dev-shared.js`, and the new `dist/observe/**`. All five granted. The
/// source of every rc.9 [`AuditedCitation::Implementation`] below; for the
/// repository owner's review, as the rc.3 and rc.6 audits were.
const RC9_SIGNALS_AUDIT: &str =
    "docs/package-contract-v2/audits/2026-09-26-solid-2-rc9-signals-negative-rows.md";

/// The reading of the other 20 rc.3 `@solidjs/signals` rows on the exact bytes
/// of `@solidjs/signals@2.0.0-rc.9` (2026-09-27), in the same three builds as
/// [`RC9_SIGNALS_AUDIT`]: 19 granted, `createOptimisticStore` `reads` withheld
/// for the rc.6 reason (the export's own landing router still reads through
/// the store proxy it created). The source of every rc.9
/// [`AuditedCitation::Implementation`] outside the five `creates` rows; for the
/// repository owner's review, as the earlier audits were.
const RC9_PARITY_AUDIT: &str =
    "docs/package-contract-v2/audits/2026-09-27-solid-2-rc9-signals-negative-rows-parity.md";

/// The reading of all 26 `solid-js`/`@solidjs/web` rows the table carried for
/// rc.3, on the exact bytes of `solid-js@2.0.0-rc.9` and
/// `@solidjs/web@2.0.0-rc.9` (2026-09-27), in all six builds each package's
/// `.` entry can select (`dist/solid{,.dev,.observe}.js` or
/// `dist/web{,.dev,.observe}.js`, and `dist/server{,.dev,.observe}.js`). 14
/// flat rows and one `browser`-scoped row granted; 12 withheld. The source of
/// every rc.9 `solid-js`/`@solidjs/web` [`AuditedCitation::Implementation`]
/// below; for the repository owner's review.
const RC9_CORE_WEB_AUDIT: &str =
    "docs/package-contract-v2/audits/2026-09-27-solid-2-rc9-core-and-web-negative-rows.md";

/// The reading of `@solidjs/signals`' `merge` and `omit` and of `solid-js`' own
/// `createMemo`, `creates` only, on the exact bytes of the rc.9 archives
/// (2026-09-28), in every build each `exports` map can select. `omit` granted
/// for every condition; `createMemo` granted scoped to `browser`, bound through
/// rc.9 signals' own `createMemo`, `createSignal` and `getOwner` rows. `merge`
/// and the flat `createMemo` withheld: `solid-js`' server builds define their
/// own `merge` and `createMemo`, which reach `ctx.serialize`.
const RC9_MERGE_OMIT_MEMO_AUDIT: &str =
    "docs/package-contract-v2/audits/2026-09-28-solid-2-rc9-merge-omit-creatememo-creates.md";

/// The reading of `solid-js`' own `createSignal` and `createMemo` in the rc.9
/// server builds (`dist/server{,.dev,.observe}.js`, what `node` selects), for
/// `creates` and `reads`, asking whether a `node`-scoped or a host-free row is
/// sound (2026-09-28). It grants nothing: `creates` reaches `ctx.serialize`
/// under `renderToStream` (probed), and host-free `reads` falls on the browser
/// builds' hydration-gate read; the `node` `reads` reading is clean but the
/// scope has no premise pinning the signals archive it constructs error
/// classes from. Every section withholds, so nothing but the derivation test
/// names it.
#[cfg(test)]
const RC9_SERVER_BUILDS_AUDIT: &str = "docs/package-contract-v2/audits/2026-09-28-solid-2-rc9-server-builds-createsignal-creatememo.md";

/// The re-reading of `solid-js@2.0.0-rc.3` rows that rested on a
/// `solid-js.json` summary alone and that rc.3's own runtime bytes contradict
/// (2026-09-27). Every section there withdraws a row, and none grants one, so
/// nothing but the derivation test names it.
#[cfg(test)]
const RC3_SHOW_LOADING_AUDIT: &str =
    "docs/package-contract-v2/audits/2026-09-27-solid-2-rc3-show-loading-withdrawals.md";

/// The version each row is keyed to. Spelled once so a row cannot name a
/// prerelease by a typo that happens to match no archive; a test pins that
/// every row's `(package, version)` names exactly one [`AUDITED_ARCHIVES`]
/// tuple.
const RC3: &str = "2.0.0-rc.3";
const RC6: &str = "2.0.0-rc.6";
const RC9: &str = "2.0.0-rc.9";

/// What the audited 2.0 documents **deny**, per archive, per canonical export,
/// per call claim domain.
///
/// # Two kinds of authority, and they are not equally mechanical
///
/// Most rows cite a normalized contract document's summary object, and the
/// citation test re-derives the closure out of the cited bytes: the bytes *are*
/// the claim. Five rows — `@solidjs/signals`' `createRoot`, `createSignal`,
/// `getOwner`, `onCleanup` and `untrack` — cite the archive's own runtime bytes
/// instead, because `solidjs-signals.json` audits twelve exports and these are
/// not among them. There the closure is a **human reading** recorded in
/// [`RC3_CORE_PRIMITIVES_AUDIT`], and what the digests pin is the *subject* of
/// that reading, not its conclusion; see [`AuditedCitation`] for the exact
/// difference and ADR 0007 for why it is stated rather than smoothed over.
/// Three more rc.3 rows (`runWithOwner`, `createContext`, `useContext`) are
/// readings of the same kind ([`RC3_OWNER_CONTEXT_AUDIT`]), and every rc.6 and
/// rc.9 row is one ([`RC6_SIGNALS_AUDIT`], [`RC9_SIGNALS_AUDIT`],
/// [`RC9_PARITY_AUDIT`]).
///
/// # Rows are archive-scoped, and three `@solidjs/signals` archives carry rows
///
/// Every row names its archive's version ([`NegativeClaimRow::version`]) and
/// answers for that archive alone. The 46 rc.3 rows answer for the three
/// `2.0.0-rc.3` archives; the 24 rc.6 rows answer for
/// `@solidjs/signals@2.0.0-rc.6`, which the ecosystem installs in place of
/// rc.3 (every `solid-js@2.0.0-rc.3` declares `@solidjs/signals: ^2.0.0-rc.3`).
/// rc.6 is a substantial rewrite, so none of its rows is a carried-over rc.3
/// reading: each was read on rc.6's bytes in all three bundles, and where rc.3
/// rests on a summary of the rc.3 bundled document the rc.6 row cites rc.6's
/// runtime bytes instead. Of the 25 rc.3 `@solidjs/signals` rows, rc.6 grants
/// 24 and **withholds `createOptimisticStore` `reads`**: rc.6's new landing
/// router (`wrapCommit` -> `stageLanding` -> `stagedApply`) reads property
/// values and keys through the store proxy the call created when its derived
/// computation commits under a retained transaction — untracked reads, which §
/// reads still counts, under a condition a flat row cannot carry.
///
/// The rc.6 rows inherit the `solid-js` server-condition caveat stated below
/// for rc.3, paired with `solid-js@2.0.0-rc.3`'s server bodies (the audit read
/// those beside rc.6); a different `solid-js` version is not covered.
///
/// The 24 rc.9 rows answer for `@solidjs/signals@2.0.0-rc.9`, the release the
/// owner is making audited (solid-primitives' `next` installs it beside
/// `solid-js@2.0.0-rc.9`). The five `creates` rows of `getOwner`, `onCleanup`,
/// `createRoot`, `untrack` and `runWithOwner` are [`RC9_SIGNALS_AUDIT`]'s; the
/// other nineteen are [`RC9_PARITY_AUDIT`]'s, which read every remaining rc.3
/// row. Each was read on rc.9's own bytes in all three builds its `exports`
/// map can select (`dist/prod/**`, `dist/dev.js` with `dist/dev-shared.js`,
/// `dist/observe/**`). Like rc.6, rc.9 **withholds `createOptimisticStore`
/// `reads`**: it keeps rc.6's landing router essentially verbatim. So rc.9
/// grants exactly what rc.6 grants. With `createSignal` `creates` granted,
/// the scoped `solid-js` `createSignal` row's delegates are answered beside
/// rc.9 as beside rc.3 and rc.6. The `solid-js` pairing covers
/// `solid-js@2.0.0-rc.9`'s `server.js`, `server.dev.js` and `server.observe.js`
/// bodies of every name it re-exports from this archive, as well as rc.3's;
/// `createRoot`, `createSignal`, `createMemo`, the store constructors and the
/// optimistic primitives are `solid-js`' own declarations in rc.9.
///
/// The rc.9 `solid-js` and `@solidjs/web` rows ([`RC9_CORE_WEB_AUDIT`]) are
/// the 26 rc.3 rows of those packages re-read on rc.9's own bytes, every
/// build cited. Granted: `For`, `Repeat`, `Match` (`reads`, `creates`),
/// `createContext`, `useContext` (`creates`), `clientOnly`, `httpHeader`,
/// `httpStatus` (`reads`, `creates`), and `createSignal` `creates` scoped to
/// `browser` (bound through rc.9 signals' own `createSignal` and `getOwner`
/// `creates` rows).
/// Withheld: `Show` (both), `Loading`, flat `createSignal`, `hydrate` and
/// `render` `reads` -- each for a reach its own closure makes -- and the six
/// `solid-js` rows for `affects`, `isPending`, `latest` and `refresh`, whose
/// rc.9 declarations and browser bodies are `@solidjs/signals`', so a
/// `solid-js` row could neither cite the body nor bind.
///
/// [`RC9_MERGE_OMIT_MEMO_AUDIT`] (2026-09-28) adds two rc.9 `creates` rows:
/// `@solidjs/signals`' `omit`, for every condition (every `solid-js` build,
/// server included, re-exports its bytes), and `solid-js`' own `createMemo`
/// scoped to `browser`, bound through rc.9 signals' `createMemo`,
/// `createSignal` and `getOwner` rows exactly as the scoped `createSignal` row
/// is. It withholds `@solidjs/signals`' `merge` and the flat `solid-js`
/// `createMemo`: `solid-js`' server builds define their own `merge` and
/// `createMemo`, and both reach `ctx.serialize`.
///
/// `RC9_SERVER_BUILDS_AUDIT` (2026-09-28) read those server bodies as the
/// subject of a `node` row and grants none. `createSignal` and `createMemo`
/// `creates` hand a thenable or async-iterable result to `ctx.serialize` under
/// `renderToStream` in all three server builds (probed; `renderToString` and
/// no context reach nothing), so no `node` row and no host-free row. `reads`
/// is clean on the server bytes, but host free is withheld for the browser
/// builds' gate read (`withHydrationGate`'s signal, read by the export's own
/// compute on the call's stack), and a `node` row would need a premise pinning
/// the `@solidjs/signals` archive whose error classes the closure constructs,
/// which a delegate-less [`HostTargetScope`] cannot state.
///
/// # `creates` and `reads`, and only those
///
/// Rows carry [`CallClaimDomain::Creates`] (75: 30 on rc.3, one of them scoped, 17 on rc.6, 28 on
/// rc.9, two of them scoped) and [`CallClaimDomain::Reads`] (36: 16 on rc.3, 7 on rc.6, 13 on rc.9). The other six kinded
/// domains are withheld wholesale, because the audited documents' closures in them are not yet
/// admissible as negative authority and each counter-example below is a defect
/// against the *audit*, not against this table:
///
/// - **`returns`.** `snapshot`'s summary is `shape: "plain"` — it hands the
///   caller a value — and closes `returns: []`. `flush` and `latest` do the
///   same. `semantic-model.md` § returns defines a `return` operation as
///   exactly "the export yielding a value to its caller", so the audits are
///   using `returns` for emission-like operations (`createMemo`'s `emission`)
///   and recording the ordinary synchronous return in `shape` instead. Until
///   that convention is reconciled with the model, the domain's closures deny
///   nothing this table can restate.
/// - **`callbacks`.** `latest`'s summary closes `callbacks: []`, and
///   `latest(fn)` calls `fn()` directly — which
///   [`Solid2::callback_owners`]' own cited reading of the runtime says, and
///   which § callbacks makes an `invoke` in `callbacks`. This crate already
///   records the divergence as intentional on the audit's side
///   (`contract_schema_exemptions`: "the normalized contract models it as a
///   read operation rather than invocation of a caller-supplied callback"),
///   which is precisely why the closure cannot be read as "invokes nothing".
/// - **`writes`, `invalidates`, `cleanups`, `disposals`.** No counter-example
///   found, and no positive review performed either. They are silent because
///   nothing here has read them, which is the only honest default: a domain is
///   added when it is audited, not when it is convenient.
///
/// # `reads`, admitted 2026-09-10
///
/// The positive review that `reads` had never had is
/// `phase21/2026-09-10-reads-census-admission-review.md`, and it turned on a
/// model question rather than on a counter-example. `Show`'s summary closes
/// `reads: []` while both its operations guard on `{arg: 0, path: ["keyed"]}`;
/// whether that is a contradiction depends on whether a *caller's* props proxy
/// access is this export's read. `semantic-model.md` § reads
/// **[Decision 2026-09-10]** settles that it is not — a proxy property access
/// is this export's read only when the export owns the proxy, on exactly
/// [`ADR 0034`](../../../../docs/adr/0034-parameter-rooted-accessor-disposition.md)'s
/// argument about whose code runs. Under that decision every audited `reads`
/// closure conforms as written, and no bundled document models a props access
/// as a read, so the closures became derivable rows.
///
/// Sixteen of the twenty derivable rows ship. `createEffect` below is
/// withheld; `Show` (its memos compute on the call's stack and read memos
/// `Show` created; RC3_SHOW_LOADING_AUDIT § 1) and `@solidjs/web`'s `render`
/// and `hydrate` (an undeclared `insertOptions` reaches a hydration gate
/// signal their own closure creates and reads; § 4, § 5) were withdrawn
/// 2026-09-27 on rc.3's own bytes. All five that the worksheet
/// left open were read against the pinned rc.3 bytes on 2026-09-10
/// (`phase21/2026-09-10-reads-negative-rows-audit-worksheet.md` § 6); four
/// cleared and one did not.
///
/// - **`@solidjs/signals`'s `flush` and `action`** ship under § reads
///   **[Decision 2026-09-10]** on authorship: `flush()` drains `globalQueue`
///   and runs computations a *third party* registered, and those reads belong
///   to whoever registered them (`@solidjs/web`'s `render` and `hydrate`
///   shipped on the same argument until 2026-09-27; see above). Everything
///   else in the bodies routes to
///   caller-supplied values (ADR 0034, ADR 0048) or to object literals the
///   code built (ADR 0044).
/// - **`solid-js`'s `createEffect`** stays **withheld**, and the reason moved.
///   Its *server* path is clean: `server.js:810`'s `serverEffect` and
///   `processResult` observe no source, and `ctx.serialize` — the reach that
///   withdrew the `creates` row — serializes a promise. The counter-example is
///   in the **client** build. Under `sharedConfig.hydrating` with
///   `options.ssrSource === "client"`, `solid.js`'s `hydratedEffect` calls
///   `withHydrationGate`, which does
///   `createSignal$1(false, { ownedWrite: true })` and hands the accessor to a
///   compute that reads it — a read of a signal **this export created**, which
///   § reads counts even though it is scheduled. A guarded reach is still a
///   reach and a row carries no condition, so the row is withheld rather than
///   qualified.
///
/// Rows do **not** make the domain certifiable on their own: `reads` is not in
/// `ClaimDomain::PROPOSABLE` and `reviewed_observation` has no `reads` entry,
/// so no proposal reaches a census and every candidate withholds for want of a
/// recipe. These rows are the terminator half, landed first.
///
/// # Deliberately silent for `creates`
///
/// - **`@solidjs/web`'s `render`** publishes `register-delegation`, a `create`
///   naming `browser-root`. A published operation is the opposite of a
///   negative row, so there is nothing to deny.
/// - **`@solidjs/web`'s `hydrate`** *is* closed `creates: []` in
///   `solidjs-web.json`, and the row is **withheld anyway**. `hydrate`'s rc.3
///   body reaches `render` on every path — the fast `_$HY.done` return, both
///   arms of the module-preload continuation, and the ordinary `try`/`finally`
///   return — and `render` calls `registerDelegatedRoot(element)`
///   unconditionally before it opens its root, the exact act the sibling
///   summary models as `register-delegation`. The audit
///   therefore contradicts itself about two functions in one document, and the
///   published bytes side with `render`. Granting the row would prove a false
///   claim; correcting the audit is a re-audit with its own review, recorded in
///   `docs/precision-backlog.md`.
/// - **`@solidjs/web`'s `createServerReference`** publishes
///   `register-reference` and `transform-reference` in both server-function
///   conditions. It is also not a canonical primitive of this dialect, so it
///   could not be a row either way.
/// - **`solid-js`'s own `createSignal`** — withheld, and the withholding is a
///   *condition* finding rather than a missing audit.
///   `solid-js/types/index.d.ts:8` re-**declares** `createSignal` (from
///   `./client/hydration.js`) instead of re-exporting `@solidjs/signals`', so a
///   `solid-js` import does not inherit the row above; and the archive's own
///   implementation differs by condition. The browser bodies perform no
///   `create` ([`RC3_CORE_PRIMITIVES_AUDIT`] § 7.3), but the
///   `node`/`worker`/`deno` body's derived overload reaches
///   `ctx.serialize(id, deferred.promise, deferStream)`
///   (`dist/server.js:558`, `:699`, `:760`, `:797`), which
///   `semantic-model.md` § creates' **[Decision 2026-09-04]** settles **is** a
///   `create`. A `(package, export, domain)` row carries no condition, so the
///   *flat* row stays withheld. Since 2026-09-25 the table is condition-aware
///   for exactly this export: one [`RowScope::HostTarget`] row states what
///   § 7.3 did establish — `browser` requested, `.` resolving to
///   `dist/solid.js` under exactly that set, and the installed
///   `@solidjs/signals` archive's own audited rows denying `createSignal` and
///   `getOwner` — and [`DialectNegativeAuthority::denies`] still answers
///   nothing for it. The census terminator replays each premise; a consumer
///   host that declared no conditions never receives a case certified under
///   `browser`.
/// - **`solid-js`'s `createEffect`** — **withdrawn** 2026-09-04, for the same
///   reason, and this is the one row the § creates decision cost.
///   `solid-js.json` closes `creates: []` for it, but that document captures
///   `browser/development` only: `dist/server.js:868-870` routes `createEffect`
///   to `serverEffect`, which calls `processResult` whenever the caller passes
///   `options.ssrSource`, and `processResult` reaches the same `ctx.serialize`.
///   The reach is guarded — `node`/`worker`/`deno` ∧ `ctx.async` (a
///   `renderToStream` context) ∧ `ssrSource ∈ {server, hybrid}` ∧ the compute
///   returning a thenable or async-iterable ∧ the owner having an id ∧ not
///   `NoHydrate` — and it is reachable *type-correctly*, because
///   `solid-js/types/client/hydration.d.ts:42` augments `EffectOptions` with
///   `ssrSource` and `:568` re-declares the export carrying it. A guarded reach
///   is still a reach, and this row cannot say "except under that guard", so it
///   is withheld rather than qualified. See the open item in ADR 0007.
/// - **Every other export of `solid-js@2.0.0-rc.3` outside its audited
///   document's ten**, and every `@solidjs/signals` export outside its twelve
///   *and* outside the five [`RC3_CORE_PRIMITIVES_AUDIT`] censused by hand. The
///   archive is audited; those exports are not. `createContext`,
///   `createRenderEffect`, `runWithOwner` and the rest have no row.
/// - **`isEqual`, `applyRef`, `renderToString`, `renderToStream`,
///   `httpHeader`/`httpStatus`' non-primitive siblings.** `isEqual`,
///   `applyRef`, `renderToString` and `renderToStream` close `creates: []`
///   in the audits but are not canonical primitives of this dialect's
///   [`TABLE`], so the vocabulary has nothing to key a row on.
///
/// # Every condition, or no row
///
/// A row exists only when *every* audited document and artifact case that
/// exports the name closes the domain empty. `clientOnly`, `httpHeader` and
/// `httpStatus` are each read from two documents — the browser-development
/// case and the node-server case — and both citations are carried.
///
/// One approximation, stated rather than hidden: the audits captured the
/// `browser/development` and `node/server` conditions of `@solidjs/web`, not
/// `browser/production`. A consumer resolving to an uncaptured condition of the
/// *same* tarball receives the row on the strength of the captured ones. The
/// identity gate binds the archive, not the condition, and closing that gap
/// needs a per-condition audit — recorded in `docs/precision-backlog.md`.
///
/// The five implementation-audited rows do **not** carry that approximation:
/// each cites all three `@solidjs/signals` bundles by name (`dist/prod/**`,
/// `dist/dev.js`, `dist/node.cjs`), which is every runtime file the archive's
/// `exports` map can select. They carry a different one, stated in
/// [`RC3_CORE_PRIMITIVES_AUDIT`] § 1.4: a consumer importing these names from
/// `solid-js` under the `node` condition runs `solid-js/dist/server.js`'s own
/// bodies while its *declaration* resolves into `@solidjs/signals`, and the
/// tier binds the declaration's archive. The audit read those server bodies
/// too (§ 3.4, § 4.3, § 5.4, § 6.3) and reached the same verdict, which is what
/// makes the rows sound on that path — but the tier cannot see the split, so
/// the rows rest on the audit having read both.
const NEGATIVE_ROWS: &[NegativeClaimRow] = &[
    // `reads` on the same audited bytes. Its drain runs computations a
    // third party registered, which § reads [Decision 2026-09-10]
    // attributes to whoever registered them.
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "action",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-c094d35ac3f70f84acaae0d933ed0c4c46071004615d4a1351a11a01bf987552",
            start_byte: 33507,
            end_byte: 38878,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "action",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-c094d35ac3f70f84acaae0d933ed0c4c46071004615d4a1351a11a01bf987552",
            start_byte: 33507,
            end_byte: 38878,
        }],
    },
    // `reads` on the same audited bytes as the `creates` row below.
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "createMemo",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-6970e6d02d81c014fd7c2ef7aee46716c95cb9aac16a28e9f8adb95ece54eab1",
            start_byte: 12806,
            end_byte: 17314,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "createMemo",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-6970e6d02d81c014fd7c2ef7aee46716c95cb9aac16a28e9f8adb95ece54eab1",
            start_byte: 12806,
            end_byte: 17314,
        }],
    },
    // `reads` on the same audited bytes as the `creates` row below.
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "createOptimistic",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-92071bb735b571320a76e500e8f0dc47db0f11df19201d2b3931d2da5d763e37",
            start_byte: 25043,
            end_byte: 27967,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "createOptimistic",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-92071bb735b571320a76e500e8f0dc47db0f11df19201d2b3931d2da5d763e37",
            start_byte: 25043,
            end_byte: 27967,
        }],
    },
    // `reads` on the same audited bytes as the `creates` row below.
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "createOptimisticStore",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-034586f31ead4bb594c03ada1202fb3b439455294d049727cb6f898c65cf5283",
            start_byte: 6985,
            end_byte: 10025,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "createOptimisticStore",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-034586f31ead4bb594c03ada1202fb3b439455294d049727cb6f898c65cf5283",
            start_byte: 6985,
            end_byte: 10025,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "createProjection",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-dc0413a1214db1eaf2875e7ee5b17addfef2043f8d01429f94081d9f41a673e5",
            start_byte: 38960,
            end_byte: 44103,
        }],
    },
    // Implementation-audited, `2026-09-04-solid-2-rc3-core-primitives-creates`
    // § 3. Three bundles cited because three conditions run three files; the
    // `import` default and `require` bodies are byte-identical, which the two
    // equal `slice_sha256` values state rather than imply.
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "createRoot",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC3_CORE_PRIMITIVES_AUDIT,
                section: "## 3. `createRoot` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/prod/core/owner.js",
                file_sha256: "d86a4959cd0eca34617b14d29514d653193d5fcd87d2857c783bf04ff4aaefe7",
                start_byte: 10535,
                end_byte: 10655,
                slice_sha256: "eefc749ba75a19179e86ce86935623ba829da692628162c809267f812583bbe3",
            },
            AuditedCitation::Implementation {
                audit: RC3_CORE_PRIMITIVES_AUDIT,
                section: "## 3. `createRoot` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/dev.js",
                file_sha256: "cc68ed0f0c5de86411555af407ac7acf4d1c10206f24bab4e1793c22553f1a79",
                start_byte: 90641,
                end_byte: 90783,
                slice_sha256: "9a66667de7c1a48f5a90e9901d994ba532da603ac763dc460c16fb8e60496fa5",
            },
            AuditedCitation::Implementation {
                audit: RC3_CORE_PRIMITIVES_AUDIT,
                section: "## 3. `createRoot` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/node.cjs",
                file_sha256: "bc0e35d32add395dc1c4dc3d6cd0fb4ea4a19bd582d68de3b44c708bb4b75c1c",
                start_byte: 58592,
                end_byte: 58712,
                slice_sha256: "eefc749ba75a19179e86ce86935623ba829da692628162c809267f812583bbe3",
            },
        ],
    },
    // Implementation-audited, § 7.2. `@solidjs/signals`' own `createSignal`
    // only — `solid-js`' re-declaration is a different implementation and is
    // withheld (§ 7.4, `IMPLEMENTATION_AUDITED`).
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "createSignal",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC3_CORE_PRIMITIVES_AUDIT,
                section: "## 7. `createSignal` — two archives, two rows",
                archive_path: "dist/prod/signals.js",
                file_sha256: "b80dc49d83f80b37a572d0fa7245866c978428e450a5f673e6e83972f9db9e8a",
                start_byte: 2517,
                end_byte: 2797,
                slice_sha256: "ce6ade13e9e77463067c7bac3727622e0ac32bd8e9bad801501a28365b9da388",
            },
            AuditedCitation::Implementation {
                audit: RC3_CORE_PRIMITIVES_AUDIT,
                section: "## 7. `createSignal` — two archives, two rows",
                archive_path: "dist/dev.js",
                file_sha256: "cc68ed0f0c5de86411555af407ac7acf4d1c10206f24bab4e1793c22553f1a79",
                start_byte: 232899,
                end_byte: 233248,
                slice_sha256: "d1292c1a9d7a916d932b8e2ae27b22c592daf612cc51ddf7848ecf64f63cd504",
            },
            AuditedCitation::Implementation {
                audit: RC3_CORE_PRIMITIVES_AUDIT,
                section: "## 7. `createSignal` — two archives, two rows",
                archive_path: "dist/node.cjs",
                file_sha256: "bc0e35d32add395dc1c4dc3d6cd0fb4ea4a19bd582d68de3b44c708bb4b75c1c",
                start_byte: 175815,
                end_byte: 176095,
                slice_sha256: "d003a64c857bac064c4f1164874f761622a5dfd9406061757ea0fb00ff152fd3",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "createStore",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-7080e21f5c75fdb8ffd32ef595390c4164a07969282c6a843573230ed36de5f5",
            start_byte: 17396,
            end_byte: 23217,
        }],
    },
    // `reads` on the same audited bytes as the `creates` row below.
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "createTrackedEffect",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-aab0640db7c783a35e1c955cbf19c22197542f3eecb3f89b28433694eb07ff6a",
            start_byte: 29857,
            end_byte: 33425,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "createTrackedEffect",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-aab0640db7c783a35e1c955cbf19c22197542f3eecb3f89b28433694eb07ff6a",
            start_byte: 29857,
            end_byte: 33425,
        }],
    },
    // `reads` on the same audited bytes. Its drain runs computations a
    // third party registered, which § reads [Decision 2026-09-10]
    // attributes to whoever registered them.
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "flush",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-00fc668bf5acaf07e617a9118eb0ef43a7dc1ba6359be1ea580793a91a76efbe",
            start_byte: 2598,
            end_byte: 6903,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "flush",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-00fc668bf5acaf07e617a9118eb0ef43a7dc1ba6359be1ea580793a91a76efbe",
            start_byte: 2598,
            end_byte: 6903,
        }],
    },
    // Implementation-audited, § 4. `getOwner` is `return context` in all three
    // bundles; the call table has zero rows.
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "getOwner",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC3_CORE_PRIMITIVES_AUDIT,
                section: "## 4. `getOwner` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/prod/core/owner.js",
                file_sha256: "d86a4959cd0eca34617b14d29514d653193d5fcd87d2857c783bf04ff4aaefe7",
                start_byte: 7691,
                end_byte: 7739,
                slice_sha256: "67fcbebd02b9e57fe095ba4af938b3b4b27e52e9ecda46862ddce141f1e4c9e2",
            },
            AuditedCitation::Implementation {
                audit: RC3_CORE_PRIMITIVES_AUDIT,
                section: "## 4. `getOwner` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/dev.js",
                file_sha256: "cc68ed0f0c5de86411555af407ac7acf4d1c10206f24bab4e1793c22553f1a79",
                start_byte: 87147,
                end_byte: 87189,
                slice_sha256: "e8cb95b765807fa14a97032551f4bbced263cc3d7837fc1258f156ffc71ba8bb",
            },
            AuditedCitation::Implementation {
                audit: RC3_CORE_PRIMITIVES_AUDIT,
                section: "## 4. `getOwner` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/node.cjs",
                file_sha256: "bc0e35d32add395dc1c4dc3d6cd0fb4ea4a19bd582d68de3b44c708bb4b75c1c",
                start_byte: 55747,
                end_byte: 55795,
                slice_sha256: "67fcbebd02b9e57fe095ba4af938b3b4b27e52e9ecda46862ddce141f1e4c9e2",
            },
        ],
    },
    // Implementation-audited, § 5. The dev slice is the whole guarded body,
    // including the two dev-only diagnostic branches, because that is the
    // definition the audit walked.
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "onCleanup",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC3_CORE_PRIMITIVES_AUDIT,
                section: "## 5. `onCleanup` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "b80dc49d83f80b37a572d0fa7245866c978428e450a5f673e6e83972f9db9e8a",
                start_byte: 2368,
                end_byte: 2421,
                slice_sha256: "89ddda3041ae80177d8bea1730aeb504ddb62f57d37956e3cfea6232f0f8925b",
            },
            AuditedCitation::Implementation {
                audit: RC3_CORE_PRIMITIVES_AUDIT,
                section: "## 5. `onCleanup` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/dev.js",
                file_sha256: "cc68ed0f0c5de86411555af407ac7acf4d1c10206f24bab4e1793c22553f1a79",
                start_byte: 231953,
                end_byte: 232799,
                slice_sha256: "f20080340dc7598692247a9944e2f59a4d823a6b24df127a17b3658622521284",
            },
            AuditedCitation::Implementation {
                audit: RC3_CORE_PRIMITIVES_AUDIT,
                section: "## 5. `onCleanup` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/node.cjs",
                file_sha256: "bc0e35d32add395dc1c4dc3d6cd0fb4ea4a19bd582d68de3b44c708bb4b75c1c",
                start_byte: 175666,
                end_byte: 175719,
                slice_sha256: "89ddda3041ae80177d8bea1730aeb504ddb62f57d37956e3cfea6232f0f8925b",
            },
        ],
    },
    // `reads` on the same audited bytes as the `creates` row below.
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "onSettled",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-5a08fc896d6f18c5378c69bc27d5fc1ddaeb013364aff1421330341801111663",
            start_byte: 10107,
            end_byte: 12724,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "onSettled",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-5a08fc896d6f18c5378c69bc27d5fc1ddaeb013364aff1421330341801111663",
            start_byte: 10107,
            end_byte: 12724,
        }],
    },
    // `reads` on the same audited bytes as the `creates` row below.
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "reconcile",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-97b25908bde1ce8220884836f97f37a42f6719bcf3b723d9de5746955fcc12dd",
            start_byte: 28049,
            end_byte: 29775,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "reconcile",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-97b25908bde1ce8220884836f97f37a42f6719bcf3b723d9de5746955fcc12dd",
            start_byte: 28049,
            end_byte: 29775,
        }],
    },
    // Implementation-audited, RC3_OWNER_CONTEXT_AUDIT § 1. The owner and
    // tracking swap around the caller's `fn`, in all three bundles; dev adds a
    // diagnostic for a disposed owner. `solid-js/dist/server.js:82-90` is read
    // there too and reaches the same verdict.
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "runWithOwner",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC3_OWNER_CONTEXT_AUDIT,
                section: "## 1. `runWithOwner` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/prod/core/core.js",
                file_sha256: "1726b40ebf79cf15b8d09ce2078a78a6a8ca71bf48e4b3ba12880736ae281a46",
                start_byte: 37382,
                end_byte: 37594,
                slice_sha256: "5c40363ecc6eaf66378b57e0c387103fe67f51706d30dab3cb041fd10f8af3e5",
            },
            AuditedCitation::Implementation {
                audit: RC3_OWNER_CONTEXT_AUDIT,
                section: "## 1. `runWithOwner` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/dev.js",
                file_sha256: "cc68ed0f0c5de86411555af407ac7acf4d1c10206f24bab4e1793c22553f1a79",
                start_byte: 172400,
                end_byte: 173046,
                slice_sha256: "87f9a23193e8b2d00afedb19b7176d741338b3c996268408c46427cf4df0035f",
            },
            AuditedCitation::Implementation {
                audit: RC3_OWNER_CONTEXT_AUDIT,
                section: "## 1. `runWithOwner` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/node.cjs",
                file_sha256: "bc0e35d32add395dc1c4dc3d6cd0fb4ea4a19bd582d68de3b44c708bb4b75c1c",
                start_byte: 125175,
                end_byte: 125387,
                slice_sha256: "5c40363ecc6eaf66378b57e0c387103fe67f51706d30dab3cb041fd10f8af3e5",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "snapshot",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-8911cd9f25cc9dc4140432201dd677dbebfb3177847e315e1aa05b9c628dde30",
            start_byte: 23299,
            end_byte: 24961,
        }],
    },
    // Implementation-audited, § 6. Prod and `node.cjs` are the same body under
    // different mangled hook-slot names, so their slice digests differ where
    // `createRoot`'s and `getOwner`'s do not.
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC3,
        export: "untrack",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC3_CORE_PRIMITIVES_AUDIT,
                section: "## 6. `untrack` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/prod/core/core.js",
                file_sha256: "1726b40ebf79cf15b8d09ce2078a78a6a8ca71bf48e4b3ba12880736ae281a46",
                start_byte: 24547,
                end_byte: 24827,
                slice_sha256: "520000b7e878116206ba2af96db7539d71ecbbba9e62b694d2a9cca18d5d6156",
            },
            AuditedCitation::Implementation {
                audit: RC3_CORE_PRIMITIVES_AUDIT,
                section: "## 6. `untrack` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/dev.js",
                file_sha256: "cc68ed0f0c5de86411555af407ac7acf4d1c10206f24bab4e1793c22553f1a79",
                start_byte: 155777,
                end_byte: 156253,
                slice_sha256: "2a929c2683ae93a3820bf2b4bf1a4455b41c6a928880eaa480a6820fe43a1852",
            },
            AuditedCitation::Implementation {
                audit: RC3_CORE_PRIMITIVES_AUDIT,
                section: "## 6. `untrack` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/node.cjs",
                file_sha256: "bc0e35d32add395dc1c4dc3d6cd0fb4ea4a19bd582d68de3b44c708bb4b75c1c",
                start_byte: 112365,
                end_byte: 112645,
                slice_sha256: "0ac3b93214231630dc9351ba9e53339b9c2624a63848fd3c6c9e4d5f2cd209ac",
            },
        ],
    },
    // `@solidjs/signals@2.0.0-rc.6`, read on its own bytes (RC6_SIGNALS_AUDIT).
    // Every row cites all three bundles the `exports` map can select
    // (`dist/prod/**`, `dist/dev.js`, `dist/node.cjs`) and none rests on the
    // rc.3 bundled document: where rc.3 cites a summary, rc.6 cites an
    // implementation reading. `createOptimisticStore` `reads` is absent on
    // purpose -- withheld by that audit's group C § 5 -- and
    // `createProjection`/`createOptimisticStore` cite `createProjectionNext`/
    // `createOptimisticStoreNext`, the definitions rc.6 exports under those
    // names (`dist/prod/index.js:35,37`, `dist/dev.js:12931,12933`,
    // `dist/node.cjs:11021,11025`).
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "action",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 1. `action` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/core/action.js",
                file_sha256: "67164bd6a93e34ba2309d4f2d16ada2005c70884e439a6bf14affbdf134b0506",
                start_byte: 3146,
                end_byte: 5879,
                slice_sha256: "d58c05845ac827bda46a9d6d061f30a5fe1804c955baa2ba855741ae6b7c81a8",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 1. `action` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 272268,
                end_byte: 275948,
                slice_sha256: "17fac3b6886553d93d15963281b8a2accc36ff6a664622797b4ed1054ce70428",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 1. `action` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 205514,
                end_byte: 208247,
                slice_sha256: "3ac24c390918cb76da7303aefb948dad59301f320b06cc147e7347cc896dd73d",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "action",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 2. `action` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/core/action.js",
                file_sha256: "67164bd6a93e34ba2309d4f2d16ada2005c70884e439a6bf14affbdf134b0506",
                start_byte: 3146,
                end_byte: 5879,
                slice_sha256: "d58c05845ac827bda46a9d6d061f30a5fe1804c955baa2ba855741ae6b7c81a8",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 2. `action` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 272268,
                end_byte: 275948,
                slice_sha256: "17fac3b6886553d93d15963281b8a2accc36ff6a664622797b4ed1054ce70428",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 2. `action` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 205514,
                end_byte: 208247,
                slice_sha256: "3ac24c390918cb76da7303aefb948dad59301f320b06cc147e7347cc896dd73d",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "createMemo",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 1. `createMemo` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "b75f46deb1ea7bae16d5d806d5e50582e68fd8b56d577cb0a1c4b17f1c351729",
                start_byte: 3016,
                end_byte: 3083,
                slice_sha256: "f0f1d1a80dd61e1c62139a524f277d95e34957723fc71a02d9b9996e26cf5951",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 1. `createMemo` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 279048,
                end_byte: 279137,
                slice_sha256: "46130d23b4ce3e9a8bfbe79e6ea1920b95f064c446f61e65327e08305592e3f0",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 1. `createMemo` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 210478,
                end_byte: 210545,
                slice_sha256: "f0f1d1a80dd61e1c62139a524f277d95e34957723fc71a02d9b9996e26cf5951",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "createMemo",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 2. `createMemo` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "b75f46deb1ea7bae16d5d806d5e50582e68fd8b56d577cb0a1c4b17f1c351729",
                start_byte: 3016,
                end_byte: 3083,
                slice_sha256: "f0f1d1a80dd61e1c62139a524f277d95e34957723fc71a02d9b9996e26cf5951",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 2. `createMemo` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 279048,
                end_byte: 279137,
                slice_sha256: "46130d23b4ce3e9a8bfbe79e6ea1920b95f064c446f61e65327e08305592e3f0",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 2. `createMemo` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 210478,
                end_byte: 210545,
                slice_sha256: "f0f1d1a80dd61e1c62139a524f277d95e34957723fc71a02d9b9996e26cf5951",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "createOptimistic",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 3. `createOptimistic` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "b75f46deb1ea7bae16d5d806d5e50582e68fd8b56d577cb0a1c4b17f1c351729",
                start_byte: 24983,
                end_byte: 25530,
                slice_sha256: "09fd4865502a92243213a9a3162d7e6a99bed5d6456d0395e5d425d44161f189",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 3. `createOptimistic` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 302445,
                end_byte: 303053,
                slice_sha256: "9180aeea6599b0c64fdfb3859e8c1802ace64c405e0d05ff952bc44e63ad7be3",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 3. `createOptimistic` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 232451,
                end_byte: 232998,
                slice_sha256: "ad4b94d8ef113c1707daf065d56e58285f2e58894bbc2f6fcbfbef9b018252f5",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "createOptimistic",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 4. `createOptimistic` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "b75f46deb1ea7bae16d5d806d5e50582e68fd8b56d577cb0a1c4b17f1c351729",
                start_byte: 24983,
                end_byte: 25530,
                slice_sha256: "09fd4865502a92243213a9a3162d7e6a99bed5d6456d0395e5d425d44161f189",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 4. `createOptimistic` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 302445,
                end_byte: 303053,
                slice_sha256: "9180aeea6599b0c64fdfb3859e8c1802ace64c405e0d05ff952bc44e63ad7be3",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 4. `createOptimistic` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 232451,
                end_byte: 232998,
                slice_sha256: "ad4b94d8ef113c1707daf065d56e58285f2e58894bbc2f6fcbfbef9b018252f5",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "createOptimisticStore",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 6. `createOptimisticStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/store/next/optimistic.js",
                file_sha256: "ffc69f0689ff495b827f472b7d851de3c732d28ce68d7ada8f625e5917f0280c",
                start_byte: 7627,
                end_byte: 13764,
                slice_sha256: "47f73121a743207425b26e3a0152fe44a62685a072ac55905fa1e280d0e29742",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 6. `createOptimisticStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 480810,
                end_byte: 487127,
                slice_sha256: "71e7e28b000575bc9b490c14d0cfac197296f66082f6dd24cae9e88705c7d83b",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 6. `createOptimisticStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 405742,
                end_byte: 411875,
                slice_sha256: "ab55fa7ae34c744956abb6ea7c6074ac34ef2347e234c0c4fe8eb18084ca4b26",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "createProjection",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 2. `createProjection` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/store/next/projection.js",
                file_sha256: "9f8365fec3e8f7ba82b376301f2b766b4615bdeb3ad305a40d793471884940d0",
                start_byte: 7048,
                end_byte: 7146,
                slice_sha256: "bcbe84ac16a3f08d0d7aca97559146257937d7387ca7a0d10af5096c18e045d7",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 2. `createProjection` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 450389,
                end_byte: 450505,
                slice_sha256: "e1264ce5581a8fe3610fd0f676925fcb8f30fc2bc21983c0a5b712952b220e32",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 2. `createProjection` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 374809,
                end_byte: 374907,
                slice_sha256: "fe1759454f424449b55f1d060c51173ce0cca1ea401cac112069f4bcc3909522",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "createRoot",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 3. `createRoot` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/core/owner.js",
                file_sha256: "39d08c35040e378f70b0751de74b3357711b9ec8d4cd786d32c502d0f75ac7bc",
                start_byte: 10535,
                end_byte: 10655,
                slice_sha256: "eefc749ba75a19179e86ce86935623ba829da692628162c809267f812583bbe3",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 3. `createRoot` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 117762,
                end_byte: 117904,
                slice_sha256: "9a66667de7c1a48f5a90e9901d994ba532da603ac763dc460c16fb8e60496fa5",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 3. `createRoot` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 76285,
                end_byte: 76405,
                slice_sha256: "eefc749ba75a19179e86ce86935623ba829da692628162c809267f812583bbe3",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "createSignal",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 5. `createSignal` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "b75f46deb1ea7bae16d5d806d5e50582e68fd8b56d577cb0a1c4b17f1c351729",
                start_byte: 2735,
                end_byte: 3015,
                slice_sha256: "ce6ade13e9e77463067c7bac3727622e0ac32bd8e9bad801501a28365b9da388",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 5. `createSignal` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 278699,
                end_byte: 279048,
                slice_sha256: "d1292c1a9d7a916d932b8e2ae27b22c592daf612cc51ddf7848ecf64f63cd504",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 5. `createSignal` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 210197,
                end_byte: 210477,
                slice_sha256: "e8f670802281f73242b3a9e1d182fa6546a957587c92c93ff17a883108eadeed",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "createStore",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 1. `createStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/store/index.js",
                file_sha256: "4c0118833a2e8fb1455313398d87b6700aeb7f9a98731f28308ece04823350e8",
                start_byte: 525,
                end_byte: 676,
                slice_sha256: "5ccb92725a12c3fe4c8eacf07bc49e364f976718010a237a995e0794377c7ff4",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 1. `createStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 513397,
                end_byte: 513583,
                slice_sha256: "5ab024879afe7976504cb2b296f2e44c5f68823cd9cfc7c07c51d1dcba7d80ae",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 1. `createStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 437700,
                end_byte: 437851,
                slice_sha256: "26c491269f4774ec20220d6b61d386d4405a2b966120e5cf6251fa9a5b941216",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "createTrackedEffect",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 3. `createTrackedEffect` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "b75f46deb1ea7bae16d5d806d5e50582e68fd8b56d577cb0a1c4b17f1c351729",
                start_byte: 8995,
                end_byte: 9063,
                slice_sha256: "eb5e053187dd25e3e410d0c6755696fcbd64cf462b00940e7181f6925ee6da30",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 3. `createTrackedEffect` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 285726,
                end_byte: 285859,
                slice_sha256: "5dcd108dadf1d96b9b86fccffa374571a8891951ae21d592424267ad769a00c0",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 3. `createTrackedEffect` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 216457,
                end_byte: 216525,
                slice_sha256: "eb5e053187dd25e3e410d0c6755696fcbd64cf462b00940e7181f6925ee6da30",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "createTrackedEffect",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 4. `createTrackedEffect` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "b75f46deb1ea7bae16d5d806d5e50582e68fd8b56d577cb0a1c4b17f1c351729",
                start_byte: 8995,
                end_byte: 9063,
                slice_sha256: "eb5e053187dd25e3e410d0c6755696fcbd64cf462b00940e7181f6925ee6da30",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 4. `createTrackedEffect` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 285726,
                end_byte: 285859,
                slice_sha256: "5dcd108dadf1d96b9b86fccffa374571a8891951ae21d592424267ad769a00c0",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 4. `createTrackedEffect` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 216457,
                end_byte: 216525,
                slice_sha256: "eb5e053187dd25e3e410d0c6755696fcbd64cf462b00940e7181f6925ee6da30",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "flush",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 7. `flush` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/core/scheduler.js",
                file_sha256: "c66bec648e3f0b02333ce2283df9fcb9f77ec88038c0507b400c7529b44bf686",
                start_byte: 41371,
                end_byte: 42076,
                slice_sha256: "e3434c901ced77df55c01c33543e0d5c54c903b449465fb7faaeff35edc4ecfe",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 7. `flush` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 95274,
                end_byte: 97566,
                slice_sha256: "b83e8e8063ba321ae0000c8c86229715cb4d6145990b5e79ca0a18a0620fd2fc",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 7. `flush` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 57834,
                end_byte: 58541,
                slice_sha256: "0c76954b44e34caf9f432a8d4097e86152f9753d788b1d100746364109c5e02d",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "flush",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 8. `flush` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/core/scheduler.js",
                file_sha256: "c66bec648e3f0b02333ce2283df9fcb9f77ec88038c0507b400c7529b44bf686",
                start_byte: 41371,
                end_byte: 42076,
                slice_sha256: "e3434c901ced77df55c01c33543e0d5c54c903b449465fb7faaeff35edc4ecfe",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 8. `flush` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 95274,
                end_byte: 97566,
                slice_sha256: "b83e8e8063ba321ae0000c8c86229715cb4d6145990b5e79ca0a18a0620fd2fc",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 8. `flush` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 57834,
                end_byte: 58541,
                slice_sha256: "0c76954b44e34caf9f432a8d4097e86152f9753d788b1d100746364109c5e02d",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "getOwner",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 1. `getOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/core/owner.js",
                file_sha256: "39d08c35040e378f70b0751de74b3357711b9ec8d4cd786d32c502d0f75ac7bc",
                start_byte: 7691,
                end_byte: 7739,
                slice_sha256: "67fcbebd02b9e57fe095ba4af938b3b4b27e52e9ecda46862ddce141f1e4c9e2",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 1. `getOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 114268,
                end_byte: 114310,
                slice_sha256: "e8cb95b765807fa14a97032551f4bbced263cc3d7837fc1258f156ffc71ba8bb",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 1. `getOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 73440,
                end_byte: 73488,
                slice_sha256: "67fcbebd02b9e57fe095ba4af938b3b4b27e52e9ecda46862ddce141f1e4c9e2",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "onCleanup",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 2. `onCleanup` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "b75f46deb1ea7bae16d5d806d5e50582e68fd8b56d577cb0a1c4b17f1c351729",
                start_byte: 2586,
                end_byte: 2639,
                slice_sha256: "89ddda3041ae80177d8bea1730aeb504ddb62f57d37956e3cfea6232f0f8925b",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 2. `onCleanup` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 277753,
                end_byte: 278599,
                slice_sha256: "f20080340dc7598692247a9944e2f59a4d823a6b24df127a17b3658622521284",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 2. `onCleanup` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 210048,
                end_byte: 210101,
                slice_sha256: "89ddda3041ae80177d8bea1730aeb504ddb62f57d37956e3cfea6232f0f8925b",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "onSettled",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 5. `onSettled` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "b75f46deb1ea7bae16d5d806d5e50582e68fd8b56d577cb0a1c4b17f1c351729",
                start_byte: 29410,
                end_byte: 29932,
                slice_sha256: "11b85618b7a9579abfb9a496bb0365c748d727e4c67f81fa8fb453d994106401",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 5. `onSettled` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 306936,
                end_byte: 308089,
                slice_sha256: "e17cf3b7eab43b6e7e3de51b3a78659624729beea1be0b63c8e996984b6efd5b",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 5. `onSettled` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 236878,
                end_byte: 237400,
                slice_sha256: "6df330cefe1150c5133020ced2c45fc059796d047b8dea7098e53bae06e2b323",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "onSettled",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 6. `onSettled` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "b75f46deb1ea7bae16d5d806d5e50582e68fd8b56d577cb0a1c4b17f1c351729",
                start_byte: 29410,
                end_byte: 29932,
                slice_sha256: "11b85618b7a9579abfb9a496bb0365c748d727e4c67f81fa8fb453d994106401",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 6. `onSettled` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 306936,
                end_byte: 308089,
                slice_sha256: "e17cf3b7eab43b6e7e3de51b3a78659624729beea1be0b63c8e996984b6efd5b",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 6. `onSettled` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 236878,
                end_byte: 237400,
                slice_sha256: "6df330cefe1150c5133020ced2c45fc059796d047b8dea7098e53bae06e2b323",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "reconcile",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 7. `reconcile` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/store/index.js",
                file_sha256: "4c0118833a2e8fb1455313398d87b6700aeb7f9a98731f28308ece04823350e8",
                start_byte: 678,
                end_byte: 758,
                slice_sha256: "d99b5e9158dfd8f7042e5b5b8aa40be80ee52ab0c1e6c95d7f3ea18b0d5d9099",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 7. `reconcile` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 513584,
                end_byte: 513682,
                slice_sha256: "572cd0645c23a657a2116f3609d220d042405d28255c1fefb439573572dbfc9c",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 7. `reconcile` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 437853,
                end_byte: 437933,
                slice_sha256: "faebd5c8615da98fc680ffec7005725db52aa637b982a45dc90934172454f68e",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "reconcile",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 8. `reconcile` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/store/index.js",
                file_sha256: "4c0118833a2e8fb1455313398d87b6700aeb7f9a98731f28308ece04823350e8",
                start_byte: 678,
                end_byte: 758,
                slice_sha256: "d99b5e9158dfd8f7042e5b5b8aa40be80ee52ab0c1e6c95d7f3ea18b0d5d9099",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 8. `reconcile` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 513584,
                end_byte: 513682,
                slice_sha256: "572cd0645c23a657a2116f3609d220d042405d28255c1fefb439573572dbfc9c",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 8. `reconcile` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 437853,
                end_byte: 437933,
                slice_sha256: "faebd5c8615da98fc680ffec7005725db52aa637b982a45dc90934172454f68e",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "runWithOwner",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 6. `runWithOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/core/core.js",
                file_sha256: "6b36cfb79c0b72f40e2b521901adbf23c90220aed4d5731602bb85c527338076",
                start_byte: 45088,
                end_byte: 45300,
                slice_sha256: "5c40363ecc6eaf66378b57e0c387103fe67f51706d30dab3cb041fd10f8af3e5",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 6. `runWithOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 214431,
                end_byte: 215077,
                slice_sha256: "87f9a23193e8b2d00afedb19b7176d741338b3c996268408c46427cf4df0035f",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 6. `runWithOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 155261,
                end_byte: 155473,
                slice_sha256: "5c40363ecc6eaf66378b57e0c387103fe67f51706d30dab3cb041fd10f8af3e5",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "snapshot",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 3. `snapshot` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/store/index.js",
                file_sha256: "4c0118833a2e8fb1455313398d87b6700aeb7f9a98731f28308ece04823350e8",
                start_byte: 760,
                end_byte: 813,
                slice_sha256: "3bb31686d1eba591133042dd4c7c95515074c440f6d532da34f25cbf6c05bfd2",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 3. `snapshot` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 513683,
                end_byte: 513742,
                slice_sha256: "eebc4206bc3509952fd02519f13f9b58ed00bc63da5658c90322c97bd221d6be",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 3. `snapshot` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 437935,
                end_byte: 437988,
                slice_sha256: "3bb31686d1eba591133042dd4c7c95515074c440f6d532da34f25cbf6c05bfd2",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC6,
        export: "untrack",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 4. `untrack` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/prod/core/core.js",
                file_sha256: "6b36cfb79c0b72f40e2b521901adbf23c90220aed4d5731602bb85c527338076",
                start_byte: 28285,
                end_byte: 28565,
                slice_sha256: "ac6c7ed9b0b05b086e8d17adb2546ddbf1a5af879a3447d12e3fe5a3b48831a6",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 4. `untrack` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/dev.js",
                file_sha256: "f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73",
                start_byte: 193515,
                end_byte: 193991,
                slice_sha256: "2a929c2683ae93a3820bf2b4bf1a4455b41c6a928880eaa480a6820fe43a1852",
            },
            AuditedCitation::Implementation {
                audit: RC6_SIGNALS_AUDIT,
                section: "### 4. `untrack` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
                archive_path: "dist/node.cjs",
                file_sha256: "4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f",
                start_byte: 138485,
                end_byte: 138765,
                slice_sha256: "7c6477036bc9e674dc99ea792fac64ac38fc9ad394f535163659a8e620bf6767",
            },
        ],
    },
    // `@solidjs/signals@2.0.0-rc.9`, read on its own bytes: the five `creates`
    // rows of RC9_SIGNALS_AUDIT (`createRoot`, `getOwner`, `onCleanup`,
    // `runWithOwner`, `untrack`) and the nineteen of RC9_PARITY_AUDIT, which
    // reads every other rc.3 row and withholds `createOptimisticStore` `reads`
    // exactly as rc.6 does. Every row cites all three builds rc.9's `exports`
    // map can select -- `dist/prod/**` (`default`), the `development`/`test`
    // build `dist/dev.js` (which imports `flush`, `getOwner`, `createRoot`,
    // `runWithOwner` and `untrack` from `dist/dev-shared.js` and defines the
    // rest itself), and the `observe` build `dist/observe/**`. rc.9 ships no
    // CommonJS bundle.
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "action",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 1. `action` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/core/action.js",
                file_sha256: "ade163f51610818458e29a7902c3b7042ce1f098ff6be252638857a848d44b77",
                start_byte: 4429,
                end_byte: 7642,
                slice_sha256: "92babceb624fbc7aff9607a87d8bb7d36fd8be20ef96a92d4d3c6f32d594f422",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 1. `action` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 87962,
                end_byte: 92525,
                slice_sha256: "6b9bd50c0f6bdfe85705563b2a76f7e146e2db559fce629be832289347893968",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 1. `action` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/core/action.js",
                file_sha256: "506c5f29d6344334f9eccece2c925dd0b94af4eaf3e0273aed825ed37ded4298",
                start_byte: 4502,
                end_byte: 8183,
                slice_sha256: "a503cb661462aed999de082d197ef9d229bd476655f176a73b1fce20603210ea",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "action",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 2. `action` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/core/action.js",
                file_sha256: "ade163f51610818458e29a7902c3b7042ce1f098ff6be252638857a848d44b77",
                start_byte: 4429,
                end_byte: 7642,
                slice_sha256: "92babceb624fbc7aff9607a87d8bb7d36fd8be20ef96a92d4d3c6f32d594f422",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 2. `action` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 87962,
                end_byte: 92525,
                slice_sha256: "6b9bd50c0f6bdfe85705563b2a76f7e146e2db559fce629be832289347893968",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 2. `action` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/core/action.js",
                file_sha256: "506c5f29d6344334f9eccece2c925dd0b94af4eaf3e0273aed825ed37ded4298",
                start_byte: 4502,
                end_byte: 8183,
                slice_sha256: "a503cb661462aed999de082d197ef9d229bd476655f176a73b1fce20603210ea",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "createMemo",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 1. `createMemo` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "d1a61ff0872b42987400100413bdb69e83e1e8e67dad175cf1178c7fb34646b6",
                start_byte: 3028,
                end_byte: 3095,
                slice_sha256: "f0f1d1a80dd61e1c62139a524f277d95e34957723fc71a02d9b9996e26cf5951",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 1. `createMemo` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 98008,
                end_byte: 98097,
                slice_sha256: "46130d23b4ce3e9a8bfbe79e6ea1920b95f064c446f61e65327e08305592e3f0",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 1. `createMemo` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/signals.js",
                file_sha256: "6a338c1513530b9c423c215723b171fda5b23f47a010037f845fd3c467ad41b5",
                start_byte: 3110,
                end_byte: 3177,
                slice_sha256: "f0f1d1a80dd61e1c62139a524f277d95e34957723fc71a02d9b9996e26cf5951",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "createMemo",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 2. `createMemo` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "d1a61ff0872b42987400100413bdb69e83e1e8e67dad175cf1178c7fb34646b6",
                start_byte: 3028,
                end_byte: 3095,
                slice_sha256: "f0f1d1a80dd61e1c62139a524f277d95e34957723fc71a02d9b9996e26cf5951",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 2. `createMemo` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 98008,
                end_byte: 98097,
                slice_sha256: "46130d23b4ce3e9a8bfbe79e6ea1920b95f064c446f61e65327e08305592e3f0",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 2. `createMemo` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/signals.js",
                file_sha256: "6a338c1513530b9c423c215723b171fda5b23f47a010037f845fd3c467ad41b5",
                start_byte: 3110,
                end_byte: 3177,
                slice_sha256: "f0f1d1a80dd61e1c62139a524f277d95e34957723fc71a02d9b9996e26cf5951",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "createOptimistic",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 3. `createOptimistic` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "d1a61ff0872b42987400100413bdb69e83e1e8e67dad175cf1178c7fb34646b6",
                start_byte: 26484,
                end_byte: 27031,
                slice_sha256: "09fd4865502a92243213a9a3162d7e6a99bed5d6456d0395e5d425d44161f189",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 3. `createOptimistic` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 122783,
                end_byte: 123391,
                slice_sha256: "9180aeea6599b0c64fdfb3859e8c1802ace64c405e0d05ff952bc44e63ad7be3",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 3. `createOptimistic` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/signals.js",
                file_sha256: "6a338c1513530b9c423c215723b171fda5b23f47a010037f845fd3c467ad41b5",
                start_byte: 26566,
                end_byte: 27147,
                slice_sha256: "760ee92d8393a3d2102625aadc0c31e9378285b46bc98c4af9b2e0fa85697fe1",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "createOptimistic",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 4. `createOptimistic` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "d1a61ff0872b42987400100413bdb69e83e1e8e67dad175cf1178c7fb34646b6",
                start_byte: 26484,
                end_byte: 27031,
                slice_sha256: "09fd4865502a92243213a9a3162d7e6a99bed5d6456d0395e5d425d44161f189",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 4. `createOptimistic` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 122783,
                end_byte: 123391,
                slice_sha256: "9180aeea6599b0c64fdfb3859e8c1802ace64c405e0d05ff952bc44e63ad7be3",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 4. `createOptimistic` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/signals.js",
                file_sha256: "6a338c1513530b9c423c215723b171fda5b23f47a010037f845fd3c467ad41b5",
                start_byte: 26566,
                end_byte: 27147,
                slice_sha256: "760ee92d8393a3d2102625aadc0c31e9378285b46bc98c4af9b2e0fa85697fe1",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "createOptimisticStore",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 6. `createOptimisticStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/store/next/optimistic.js",
                file_sha256: "b3a3de0d67305b92c398eca0602f1a72f350e0d6915dc8d12cf486484b4e964c",
                start_byte: 6996,
                end_byte: 13900,
                slice_sha256: "803fcb9cc11f9c2c503c6d6f86a6d82434f3655b8a1264fff1d3ed2d36b6a0a0",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 6. `createOptimisticStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 342913,
                end_byte: 349941,
                slice_sha256: "bc5b1564c404e2ba7b688f6f44bb56fc3b6a211b0b055c935b1561d9f7474cc6",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 6. `createOptimisticStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/store/next/optimistic.js",
                file_sha256: "3213841e222127d05061bb483e5eb0d7e3b269683023a18525dd5c076a0a55cc",
                start_byte: 7025,
                end_byte: 14010,
                slice_sha256: "ccd36eaea185d28df29b219512185424d91c1688bd6dd7b67efae16123719c30",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "createProjection",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 2. `createProjection` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/store/next/projection.js",
                file_sha256: "7e2175a020d7d9208211a122164a9c5e8bfe2c3df9aa2ec3441bcc8867982aec",
                start_byte: 7209,
                end_byte: 7307,
                slice_sha256: "bcbe84ac16a3f08d0d7aca97559146257937d7387ca7a0d10af5096c18e045d7",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 2. `createProjection` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 334649,
                end_byte: 334765,
                slice_sha256: "e1264ce5581a8fe3610fd0f676925fcb8f30fc2bc21983c0a5b712952b220e32",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 2. `createProjection` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/store/next/projection.js",
                file_sha256: "8fdf22ff1154b5183d4e1c6fb84b40e5e4decea5c90e34ca1de50fe7930634fd",
                start_byte: 7303,
                end_byte: 7401,
                slice_sha256: "bcbe84ac16a3f08d0d7aca97559146257937d7387ca7a0d10af5096c18e045d7",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "createRoot",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_SIGNALS_AUDIT,
                section: "### 3. `createRoot` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/core/owner.js",
                file_sha256: "a6b4d87b97f2d8021224d343a28bccf77ef2a9be8ba6872d91cfaa8b29dfc36e",
                start_byte: 11782,
                end_byte: 11902,
                slice_sha256: "eefc749ba75a19179e86ce86935623ba829da692628162c809267f812583bbe3",
            },
            AuditedCitation::Implementation {
                audit: RC9_SIGNALS_AUDIT,
                section: "### 3. `createRoot` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev-shared.js",
                file_sha256: "70b88ba97dcb1107878ccc161cd00651b3cff09d7e17aee5443f1cbf9689463e",
                start_byte: 135728,
                end_byte: 135870,
                slice_sha256: "9a66667de7c1a48f5a90e9901d994ba532da603ac763dc460c16fb8e60496fa5",
            },
            AuditedCitation::Implementation {
                audit: RC9_SIGNALS_AUDIT,
                section: "### 3. `createRoot` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/core/owner.js",
                file_sha256: "c384c5ab163cb53e1a611ce76bf9e2b27c8a7e7884d7a9c4ec7dfebf0f5e4126",
                start_byte: 11828,
                end_byte: 11948,
                slice_sha256: "d266035c22be8514767dbee4fe3aa0c947336ad4da585e24b09ee1722d916259",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "createSignal",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 1. `createSignal` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "d1a61ff0872b42987400100413bdb69e83e1e8e67dad175cf1178c7fb34646b6",
                start_byte: 2747,
                end_byte: 3027,
                slice_sha256: "ce6ade13e9e77463067c7bac3727622e0ac32bd8e9bad801501a28365b9da388",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 1. `createSignal` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 97659,
                end_byte: 98008,
                slice_sha256: "d1292c1a9d7a916d932b8e2ae27b22c592daf612cc51ddf7848ecf64f63cd504",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 1. `createSignal` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/signals.js",
                file_sha256: "6a338c1513530b9c423c215723b171fda5b23f47a010037f845fd3c467ad41b5",
                start_byte: 2795,
                end_byte: 3109,
                slice_sha256: "0366fccdf03f70ca98cd1678d74800abeed9109c5499c65c626ee6868e6f565f",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "createStore",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 1. `createStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/store/index.js",
                file_sha256: "4c0118833a2e8fb1455313398d87b6700aeb7f9a98731f28308ece04823350e8",
                start_byte: 525,
                end_byte: 676,
                slice_sha256: "5ccb92725a12c3fe4c8eacf07bc49e364f976718010a237a995e0794377c7ff4",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 1. `createStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 370517,
                end_byte: 370703,
                slice_sha256: "5ab024879afe7976504cb2b296f2e44c5f68823cd9cfc7c07c51d1dcba7d80ae",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 1. `createStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/store/index.js",
                file_sha256: "a58b551e6a02e139e89ac04e29b1ef08cfb086c5f55682039a31bb9b3350f324",
                start_byte: 551,
                end_byte: 702,
                slice_sha256: "5ccb92725a12c3fe4c8eacf07bc49e364f976718010a237a995e0794377c7ff4",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "createTrackedEffect",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 3. `createTrackedEffect` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "d1a61ff0872b42987400100413bdb69e83e1e8e67dad175cf1178c7fb34646b6",
                start_byte: 9691,
                end_byte: 9759,
                slice_sha256: "eb5e053187dd25e3e410d0c6755696fcbd64cf462b00940e7181f6925ee6da30",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 3. `createTrackedEffect` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 105290,
                end_byte: 105376,
                slice_sha256: "59effb18726430b691223730bbeffeb8cb7c60f6b546261051389e4ee557154e",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 3. `createTrackedEffect` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/signals.js",
                file_sha256: "6a338c1513530b9c423c215723b171fda5b23f47a010037f845fd3c467ad41b5",
                start_byte: 9773,
                end_byte: 9841,
                slice_sha256: "eb5e053187dd25e3e410d0c6755696fcbd64cf462b00940e7181f6925ee6da30",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "createTrackedEffect",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 4. `createTrackedEffect` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "d1a61ff0872b42987400100413bdb69e83e1e8e67dad175cf1178c7fb34646b6",
                start_byte: 9691,
                end_byte: 9759,
                slice_sha256: "eb5e053187dd25e3e410d0c6755696fcbd64cf462b00940e7181f6925ee6da30",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 4. `createTrackedEffect` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 105290,
                end_byte: 105376,
                slice_sha256: "59effb18726430b691223730bbeffeb8cb7c60f6b546261051389e4ee557154e",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 4. `createTrackedEffect` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/signals.js",
                file_sha256: "6a338c1513530b9c423c215723b171fda5b23f47a010037f845fd3c467ad41b5",
                start_byte: 9773,
                end_byte: 9841,
                slice_sha256: "eb5e053187dd25e3e410d0c6755696fcbd64cf462b00940e7181f6925ee6da30",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "flush",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 7. `flush` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/core/scheduler.js",
                file_sha256: "ac77e8c1c6b44a943310bb3976d41b2e66cdd58a8805dad70d7c5e7c91039cde",
                start_byte: 57644,
                end_byte: 59114,
                slice_sha256: "06c46ce4cc1dfc50fed5772ae6f737788defd6f59d26d9bc1ab81eba2d2e6536",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 7. `flush` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev-shared.js",
                file_sha256: "70b88ba97dcb1107878ccc161cd00651b3cff09d7e17aee5443f1cbf9689463e",
                start_byte: 103188,
                end_byte: 107202,
                slice_sha256: "b77e8fd032473ce4ab08aadd107ab2d9e56ef76452366d9f7e0151614c538cce",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 7. `flush` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/core/scheduler.js",
                file_sha256: "3abec8dc70d0ed2834a5b7040ea0e28b0af867c4201ecb4c90b46581020327ca",
                start_byte: 58268,
                end_byte: 60335,
                slice_sha256: "4c7dcc9dedee59eb7249051b3460fbc7a4a537fab9e489aab1293de9d4e387c0",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "flush",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 8. `flush` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/core/scheduler.js",
                file_sha256: "ac77e8c1c6b44a943310bb3976d41b2e66cdd58a8805dad70d7c5e7c91039cde",
                start_byte: 57644,
                end_byte: 59114,
                slice_sha256: "06c46ce4cc1dfc50fed5772ae6f737788defd6f59d26d9bc1ab81eba2d2e6536",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 8. `flush` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev-shared.js",
                file_sha256: "70b88ba97dcb1107878ccc161cd00651b3cff09d7e17aee5443f1cbf9689463e",
                start_byte: 103188,
                end_byte: 107202,
                slice_sha256: "b77e8fd032473ce4ab08aadd107ab2d9e56ef76452366d9f7e0151614c538cce",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 8. `flush` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/core/scheduler.js",
                file_sha256: "3abec8dc70d0ed2834a5b7040ea0e28b0af867c4201ecb4c90b46581020327ca",
                start_byte: 58268,
                end_byte: 60335,
                slice_sha256: "4c7dcc9dedee59eb7249051b3460fbc7a4a537fab9e489aab1293de9d4e387c0",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "getOwner",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_SIGNALS_AUDIT,
                section: "### 1. `getOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/core/owner.js",
                file_sha256: "a6b4d87b97f2d8021224d343a28bccf77ef2a9be8ba6872d91cfaa8b29dfc36e",
                start_byte: 8615,
                end_byte: 8663,
                slice_sha256: "67fcbebd02b9e57fe095ba4af938b3b4b27e52e9ecda46862ddce141f1e4c9e2",
            },
            AuditedCitation::Implementation {
                audit: RC9_SIGNALS_AUDIT,
                section: "### 1. `getOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev-shared.js",
                file_sha256: "70b88ba97dcb1107878ccc161cd00651b3cff09d7e17aee5443f1cbf9689463e",
                start_byte: 131908,
                end_byte: 131950,
                slice_sha256: "e8cb95b765807fa14a97032551f4bbced263cc3d7837fc1258f156ffc71ba8bb",
            },
            AuditedCitation::Implementation {
                audit: RC9_SIGNALS_AUDIT,
                section: "### 1. `getOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/core/owner.js",
                file_sha256: "c384c5ab163cb53e1a611ce76bf9e2b27c8a7e7884d7a9c4ec7dfebf0f5e4126",
                start_byte: 8635,
                end_byte: 8683,
                slice_sha256: "67fcbebd02b9e57fe095ba4af938b3b4b27e52e9ecda46862ddce141f1e4c9e2",
            },
        ],
    },
    // RC9_MERGE_OMIT_MEMO_AUDIT § 1. Every `solid-js@2.0.0-rc.9` build,
    // server included, re-exports these bytes, so the row needs no pairing
    // caveat; its sibling `merge` is withheld because the server builds do not
    // (§ 2).
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "omit",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_MERGE_OMIT_MEMO_AUDIT,
                section: "## 1. `omit` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/store/utils.js",
                file_sha256: "d6c52514b1b1010a648bd01d926f252e6788d96d4ab3cf345879e7af3032cc02",
                start_byte: 40808,
                end_byte: 42411,
                slice_sha256: "8b9a50f9840bd11547765d1269cb10f5f3486b49679852d061f1906408ca9128",
            },
            AuditedCitation::Implementation {
                audit: RC9_MERGE_OMIT_MEMO_AUDIT,
                section: "## 1. `omit` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 192223,
                end_byte: 194027,
                slice_sha256: "578fbc1412c64da8a3fa7d5ae5c7a703eb2028cf86ee29c132a1847e91bb737a",
            },
            AuditedCitation::Implementation {
                audit: RC9_MERGE_OMIT_MEMO_AUDIT,
                section: "## 1. `omit` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/store/utils.js",
                file_sha256: "57bd964e0a80aaf73cc9315105cba2be6a5fd7ef437b7733ed07056107acce2a",
                start_byte: 40834,
                end_byte: 42437,
                slice_sha256: "8b9a50f9840bd11547765d1269cb10f5f3486b49679852d061f1906408ca9128",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "onCleanup",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_SIGNALS_AUDIT,
                section: "### 2. `onCleanup` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "d1a61ff0872b42987400100413bdb69e83e1e8e67dad175cf1178c7fb34646b6",
                start_byte: 2598,
                end_byte: 2651,
                slice_sha256: "89ddda3041ae80177d8bea1730aeb504ddb62f57d37956e3cfea6232f0f8925b",
            },
            AuditedCitation::Implementation {
                audit: RC9_SIGNALS_AUDIT,
                section: "### 2. `onCleanup` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 96698,
                end_byte: 97559,
                slice_sha256: "5f8d40b1c3165f17bc00846b5063eb55f8bdd0a6b49dceba4a60f5aeb6570e44",
            },
            AuditedCitation::Implementation {
                audit: RC9_SIGNALS_AUDIT,
                section: "### 2. `onCleanup` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/signals.js",
                file_sha256: "6a338c1513530b9c423c215723b171fda5b23f47a010037f845fd3c467ad41b5",
                start_byte: 2646,
                end_byte: 2699,
                slice_sha256: "89ddda3041ae80177d8bea1730aeb504ddb62f57d37956e3cfea6232f0f8925b",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "onSettled",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 5. `onSettled` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "d1a61ff0872b42987400100413bdb69e83e1e8e67dad175cf1178c7fb34646b6",
                start_byte: 30911,
                end_byte: 32079,
                slice_sha256: "107240ae071701f70c6f27055f0a525fdcffcf0a4f98fc7b655f04758e8a43c9",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 5. `onSettled` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 127274,
                end_byte: 129069,
                slice_sha256: "add35758c224c4e5339e7e89db7000bce0b2b92ba37a28df5042414a2719547e",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 5. `onSettled` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/signals.js",
                file_sha256: "6a338c1513530b9c423c215723b171fda5b23f47a010037f845fd3c467ad41b5",
                start_byte: 31027,
                end_byte: 32219,
                slice_sha256: "7535e81cf25f0d998b1c69401f2225a460c6d4fd453855c905753c6a86471193",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "onSettled",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 6. `onSettled` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "d1a61ff0872b42987400100413bdb69e83e1e8e67dad175cf1178c7fb34646b6",
                start_byte: 30911,
                end_byte: 32079,
                slice_sha256: "107240ae071701f70c6f27055f0a525fdcffcf0a4f98fc7b655f04758e8a43c9",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 6. `onSettled` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 127274,
                end_byte: 129069,
                slice_sha256: "add35758c224c4e5339e7e89db7000bce0b2b92ba37a28df5042414a2719547e",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 6. `onSettled` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/signals.js",
                file_sha256: "6a338c1513530b9c423c215723b171fda5b23f47a010037f845fd3c467ad41b5",
                start_byte: 31027,
                end_byte: 32219,
                slice_sha256: "7535e81cf25f0d998b1c69401f2225a460c6d4fd453855c905753c6a86471193",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "reconcile",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 7. `reconcile` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/store/index.js",
                file_sha256: "4c0118833a2e8fb1455313398d87b6700aeb7f9a98731f28308ece04823350e8",
                start_byte: 678,
                end_byte: 758,
                slice_sha256: "d99b5e9158dfd8f7042e5b5b8aa40be80ee52ab0c1e6c95d7f3ea18b0d5d9099",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 7. `reconcile` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 370704,
                end_byte: 370802,
                slice_sha256: "572cd0645c23a657a2116f3609d220d042405d28255c1fefb439573572dbfc9c",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 7. `reconcile` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/store/index.js",
                file_sha256: "a58b551e6a02e139e89ac04e29b1ef08cfb086c5f55682039a31bb9b3350f324",
                start_byte: 704,
                end_byte: 784,
                slice_sha256: "d99b5e9158dfd8f7042e5b5b8aa40be80ee52ab0c1e6c95d7f3ea18b0d5d9099",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "reconcile",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 8. `reconcile` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/store/index.js",
                file_sha256: "4c0118833a2e8fb1455313398d87b6700aeb7f9a98731f28308ece04823350e8",
                start_byte: 678,
                end_byte: 758,
                slice_sha256: "d99b5e9158dfd8f7042e5b5b8aa40be80ee52ab0c1e6c95d7f3ea18b0d5d9099",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 8. `reconcile` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 370704,
                end_byte: 370802,
                slice_sha256: "572cd0645c23a657a2116f3609d220d042405d28255c1fefb439573572dbfc9c",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 8. `reconcile` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/store/index.js",
                file_sha256: "a58b551e6a02e139e89ac04e29b1ef08cfb086c5f55682039a31bb9b3350f324",
                start_byte: 704,
                end_byte: 784,
                slice_sha256: "d99b5e9158dfd8f7042e5b5b8aa40be80ee52ab0c1e6c95d7f3ea18b0d5d9099",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "runWithOwner",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_SIGNALS_AUDIT,
                section: "### 5. `runWithOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/core/core.js",
                file_sha256: "4baa2f64e47621423c0246529d3ce1b56ef82a8274aa54dfc1d345141158d53d",
                start_byte: 87558,
                end_byte: 87770,
                slice_sha256: "5c40363ecc6eaf66378b57e0c387103fe67f51706d30dab3cb041fd10f8af3e5",
            },
            AuditedCitation::Implementation {
                audit: RC9_SIGNALS_AUDIT,
                section: "### 5. `runWithOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev-shared.js",
                file_sha256: "70b88ba97dcb1107878ccc161cd00651b3cff09d7e17aee5443f1cbf9689463e",
                start_byte: 287156,
                end_byte: 287864,
                slice_sha256: "332a218ceec024a1d0c3464213b7d043d4a902c529b5f02a7d6ae0467070a11c",
            },
            AuditedCitation::Implementation {
                audit: RC9_SIGNALS_AUDIT,
                section: "### 5. `runWithOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/core/core.js",
                file_sha256: "5b2dba3ad755fce3a52b6d1a788dd3db9b720bd03193cc16bcd909958208adbe",
                start_byte: 89058,
                end_byte: 89270,
                slice_sha256: "5c40363ecc6eaf66378b57e0c387103fe67f51706d30dab3cb041fd10f8af3e5",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "snapshot",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 3. `snapshot` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/store/index.js",
                file_sha256: "4c0118833a2e8fb1455313398d87b6700aeb7f9a98731f28308ece04823350e8",
                start_byte: 760,
                end_byte: 813,
                slice_sha256: "3bb31686d1eba591133042dd4c7c95515074c440f6d532da34f25cbf6c05bfd2",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 3. `snapshot` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev.js",
                file_sha256: "f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120",
                start_byte: 370803,
                end_byte: 370862,
                slice_sha256: "eebc4206bc3509952fd02519f13f9b58ed00bc63da5658c90322c97bd221d6be",
            },
            AuditedCitation::Implementation {
                audit: RC9_PARITY_AUDIT,
                section: "### 3. `snapshot` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/store/index.js",
                file_sha256: "a58b551e6a02e139e89ac04e29b1ef08cfb086c5f55682039a31bb9b3350f324",
                start_byte: 786,
                end_byte: 839,
                slice_sha256: "3bb31686d1eba591133042dd4c7c95515074c440f6d532da34f25cbf6c05bfd2",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        version: RC9,
        export: "untrack",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_SIGNALS_AUDIT,
                section: "### 4. `untrack` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/prod/core/core.js",
                file_sha256: "4baa2f64e47621423c0246529d3ce1b56ef82a8274aa54dfc1d345141158d53d",
                start_byte: 49018,
                end_byte: 49298,
                slice_sha256: "28c2d9f3861b28ad08edaeb66a6d8f2caa5530d6e7cff226afda0a9a7b5d912a",
            },
            AuditedCitation::Implementation {
                audit: RC9_SIGNALS_AUDIT,
                section: "### 4. `untrack` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/dev-shared.js",
                file_sha256: "70b88ba97dcb1107878ccc161cd00651b3cff09d7e17aee5443f1cbf9689463e",
                start_byte: 242868,
                end_byte: 243344,
                slice_sha256: "2a929c2683ae93a3820bf2b4bf1a4455b41c6a928880eaa480a6820fe43a1852",
            },
            AuditedCitation::Implementation {
                audit: RC9_SIGNALS_AUDIT,
                section: "### 4. `untrack` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
                archive_path: "dist/observe/core/core.js",
                file_sha256: "5b2dba3ad755fce3a52b6d1a788dd3db9b720bd03193cc16bcd909958208adbe",
                start_byte: 50382,
                end_byte: 50662,
                slice_sha256: "01672a8cba1c1d7a8800b0effde85a96cffd51ac0bb7025b40b208f805e98773",
            },
        ],
    },
    // `reads` on the same audited bytes as the `creates` row below.
    // Its only argument-path atoms are parameter-rooted, which
    // `semantic-model.md` § reads [Decision 2026-09-10] assigns to the
    // caller; the closure denies a read of a proxy this export owns.
    NegativeClaimRow {
        package: "@solidjs/web",
        version: RC3,
        export: "clientOnly",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Summary {
                document: "pkg/contracts/bundled/solid-v2/solidjs-web.json",
                summary: "summary-33b1f252bf40ccbf4afd30c2d3c9e9cc74f10c3ece12fee903d628e8da7c231e",
                start_byte: 1987,
                end_byte: 9470,
            },
            AuditedCitation::Summary {
                document: "pkg/contracts/bundled/solid-v2/solidjs-web--web-node-server.json",
                summary: "summary-483140acbc18aaa00d3db45337fdd8fd31997eceec74fb9a398e49abc7990ebf",
                start_byte: 8958,
                end_byte: 10326,
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/web",
        version: RC3,
        export: "clientOnly",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Summary {
                document: "pkg/contracts/bundled/solid-v2/solidjs-web.json",
                summary: "summary-33b1f252bf40ccbf4afd30c2d3c9e9cc74f10c3ece12fee903d628e8da7c231e",
                start_byte: 1987,
                end_byte: 9470,
            },
            AuditedCitation::Summary {
                document: "pkg/contracts/bundled/solid-v2/solidjs-web--web-node-server.json",
                summary: "summary-483140acbc18aaa00d3db45337fdd8fd31997eceec74fb9a398e49abc7990ebf",
                start_byte: 8958,
                end_byte: 10326,
            },
        ],
    },
    // `reads` on the same audited bytes as the `creates` row below.
    NegativeClaimRow {
        package: "@solidjs/web",
        version: RC3,
        export: "httpHeader",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Summary {
                document: "pkg/contracts/bundled/solid-v2/solidjs-web.json",
                summary: "summary-f9d972edede76e2b96c176a3b0f83f6b9ae4f5528398a134a88373baf12ad54f",
                start_byte: 22024,
                end_byte: 22535,
            },
            AuditedCitation::Summary {
                document: "pkg/contracts/bundled/solid-v2/solidjs-web--web-node-server.json",
                summary: "summary-0ffbf4d4d8bc911a206487e2fea11778b4de8be1ba653b20a75bc8148856ec37",
                start_byte: 1875,
                end_byte: 4783,
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/web",
        version: RC3,
        export: "httpHeader",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Summary {
                document: "pkg/contracts/bundled/solid-v2/solidjs-web.json",
                summary: "summary-f9d972edede76e2b96c176a3b0f83f6b9ae4f5528398a134a88373baf12ad54f",
                start_byte: 22024,
                end_byte: 22535,
            },
            AuditedCitation::Summary {
                document: "pkg/contracts/bundled/solid-v2/solidjs-web--web-node-server.json",
                summary: "summary-0ffbf4d4d8bc911a206487e2fea11778b4de8be1ba653b20a75bc8148856ec37",
                start_byte: 1875,
                end_byte: 4783,
            },
        ],
    },
    // `reads` on the same audited bytes as the `creates` row below.
    NegativeClaimRow {
        package: "@solidjs/web",
        version: RC3,
        export: "httpStatus",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Summary {
                document: "pkg/contracts/bundled/solid-v2/solidjs-web.json",
                summary: "summary-f9d972edede76e2b96c176a3b0f83f6b9ae4f5528398a134a88373baf12ad54f",
                start_byte: 22024,
                end_byte: 22535,
            },
            AuditedCitation::Summary {
                document: "pkg/contracts/bundled/solid-v2/solidjs-web--web-node-server.json",
                summary: "summary-0ffbf4d4d8bc911a206487e2fea11778b4de8be1ba653b20a75bc8148856ec37",
                start_byte: 1875,
                end_byte: 4783,
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/web",
        version: RC3,
        export: "httpStatus",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Summary {
                document: "pkg/contracts/bundled/solid-v2/solidjs-web.json",
                summary: "summary-f9d972edede76e2b96c176a3b0f83f6b9ae4f5528398a134a88373baf12ad54f",
                start_byte: 22024,
                end_byte: 22535,
            },
            AuditedCitation::Summary {
                document: "pkg/contracts/bundled/solid-v2/solidjs-web--web-node-server.json",
                summary: "summary-0ffbf4d4d8bc911a206487e2fea11778b4de8be1ba653b20a75bc8148856ec37",
                start_byte: 1875,
                end_byte: 4783,
            },
        ],
    },
    // `hydrate` `reads` is withdrawn (RC3_SHOW_LOADING_AUDIT § 5): it turns
    // hydration on and hands its `options` to `render`, whose `reads` row fell
    // on the hydration gate its own closure creates and reads (§ 4).
    // `render` `reads` is withdrawn (RC3_SHOW_LOADING_AUDIT § 4): under
    // hydration, `options.insertOptions` reaches `insert`'s effect, and with
    // `{ scope: true, ssrSource: "client" }` `solid-js`' `hydratedEffect`
    // creates a gate signal that a compute it authored reads on `render`'s
    // own stack.
    // `@solidjs/web@2.0.0-rc.9`, read on its own bytes (RC9_CORE_WEB_AUDIT) in all
    // six builds its `.` entry can select. `clientOnly` allocates a signal and
    // starts the caller's import at the call; everything else runs inside the
    // returned component, a later invocation (§ 15, § 16). `hydrate` and `render`
    // `reads` are withheld on rc.9 (§ 21, § 22).
    NegativeClaimRow {
        package: "@solidjs/web",
        version: RC9,
        export: "clientOnly",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 15. `clientOnly` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.js",
                file_sha256: "32083d72a93231d8919bc78c57dea48d2c13bac8b571f07900c1b15fe95edfbf",
                start_byte: 75603,
                end_byte: 76441,
                slice_sha256: "f6412c9af0fb9a131456f7aa9bb9e4e7b372a6ccd58038a058e30787fe8d572b",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 15. `clientOnly` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.dev.js",
                file_sha256: "bcbe02189e31a7ce29852ef74b9a12e7ad135d92a269cb89057d112e8075129d",
                start_byte: 83888,
                end_byte: 84726,
                slice_sha256: "f6412c9af0fb9a131456f7aa9bb9e4e7b372a6ccd58038a058e30787fe8d572b",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 15. `clientOnly` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.observe.js",
                file_sha256: "b5b2950b906244f10ed54fc11a45076bcabd341f77e86249cb2ce462e72d8d74",
                start_byte: 76567,
                end_byte: 77405,
                slice_sha256: "f6412c9af0fb9a131456f7aa9bb9e4e7b372a6ccd58038a058e30787fe8d572b",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 15. `clientOnly` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.js",
                file_sha256: "b96fc8399d4c9909fb8a275e76abde586c843d006dcb427fe4a56620bfee2c40",
                start_byte: 128372,
                end_byte: 128553,
                slice_sha256: "31fd1a14ca31fa876e0af88c56d8b5c397fefd7f8ef84133d9484d5721ad1061",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 15. `clientOnly` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.dev.js",
                file_sha256: "4636ed7864cad6f1b541d70872f51d1d98c35bb21bb30549a2b0c091570465c6",
                start_byte: 139370,
                end_byte: 139551,
                slice_sha256: "31fd1a14ca31fa876e0af88c56d8b5c397fefd7f8ef84133d9484d5721ad1061",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 15. `clientOnly` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.observe.js",
                file_sha256: "9919deaaa0c6b73d1f760247581e351c2d0778fc18784653b11579c4e11fe3fa",
                start_byte: 131399,
                end_byte: 131580,
                slice_sha256: "31fd1a14ca31fa876e0af88c56d8b5c397fefd7f8ef84133d9484d5721ad1061",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/web",
        version: RC9,
        export: "clientOnly",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 16. `clientOnly` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.js",
                file_sha256: "32083d72a93231d8919bc78c57dea48d2c13bac8b571f07900c1b15fe95edfbf",
                start_byte: 75603,
                end_byte: 76441,
                slice_sha256: "f6412c9af0fb9a131456f7aa9bb9e4e7b372a6ccd58038a058e30787fe8d572b",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 16. `clientOnly` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.dev.js",
                file_sha256: "bcbe02189e31a7ce29852ef74b9a12e7ad135d92a269cb89057d112e8075129d",
                start_byte: 83888,
                end_byte: 84726,
                slice_sha256: "f6412c9af0fb9a131456f7aa9bb9e4e7b372a6ccd58038a058e30787fe8d572b",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 16. `clientOnly` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.observe.js",
                file_sha256: "b5b2950b906244f10ed54fc11a45076bcabd341f77e86249cb2ce462e72d8d74",
                start_byte: 76567,
                end_byte: 77405,
                slice_sha256: "f6412c9af0fb9a131456f7aa9bb9e4e7b372a6ccd58038a058e30787fe8d572b",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 16. `clientOnly` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.js",
                file_sha256: "b96fc8399d4c9909fb8a275e76abde586c843d006dcb427fe4a56620bfee2c40",
                start_byte: 128372,
                end_byte: 128553,
                slice_sha256: "31fd1a14ca31fa876e0af88c56d8b5c397fefd7f8ef84133d9484d5721ad1061",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 16. `clientOnly` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.dev.js",
                file_sha256: "4636ed7864cad6f1b541d70872f51d1d98c35bb21bb30549a2b0c091570465c6",
                start_byte: 139370,
                end_byte: 139551,
                slice_sha256: "31fd1a14ca31fa876e0af88c56d8b5c397fefd7f8ef84133d9484d5721ad1061",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 16. `clientOnly` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.observe.js",
                file_sha256: "9919deaaa0c6b73d1f760247581e351c2d0778fc18784653b11579c4e11fe3fa",
                start_byte: 131399,
                end_byte: 131580,
                slice_sha256: "31fd1a14ca31fa876e0af88c56d8b5c397fefd7f8ef84133d9484d5721ad1061",
            },
        ],
    },
    // Client `{}`; server: a write to the existing response and a retracting
    // cleanup, the rc.3 node-server summary's `declare-response` (§ 17, § 18).
    NegativeClaimRow {
        package: "@solidjs/web",
        version: RC9,
        export: "httpHeader",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 17. `httpHeader` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.js",
                file_sha256: "32083d72a93231d8919bc78c57dea48d2c13bac8b571f07900c1b15fe95edfbf",
                start_byte: 76479,
                end_byte: 76526,
                slice_sha256: "cd98272f0f010dfe4990526fc6ba5a3508aea05b6f10233e5747ecd803b76e42",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 17. `httpHeader` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.dev.js",
                file_sha256: "bcbe02189e31a7ce29852ef74b9a12e7ad135d92a269cb89057d112e8075129d",
                start_byte: 84764,
                end_byte: 84811,
                slice_sha256: "cd98272f0f010dfe4990526fc6ba5a3508aea05b6f10233e5747ecd803b76e42",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 17. `httpHeader` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.observe.js",
                file_sha256: "b5b2950b906244f10ed54fc11a45076bcabd341f77e86249cb2ce462e72d8d74",
                start_byte: 77443,
                end_byte: 77490,
                slice_sha256: "cd98272f0f010dfe4990526fc6ba5a3508aea05b6f10233e5747ecd803b76e42",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 17. `httpHeader` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.js",
                file_sha256: "b96fc8399d4c9909fb8a275e76abde586c843d006dcb427fe4a56620bfee2c40",
                start_byte: 129688,
                end_byte: 131088,
                slice_sha256: "3d0445e29f9bdff71a0501f0a5fff30a15073ae3955a0519708d97324c004f20",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 17. `httpHeader` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.dev.js",
                file_sha256: "4636ed7864cad6f1b541d70872f51d1d98c35bb21bb30549a2b0c091570465c6",
                start_byte: 140686,
                end_byte: 142086,
                slice_sha256: "3d0445e29f9bdff71a0501f0a5fff30a15073ae3955a0519708d97324c004f20",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 17. `httpHeader` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.observe.js",
                file_sha256: "9919deaaa0c6b73d1f760247581e351c2d0778fc18784653b11579c4e11fe3fa",
                start_byte: 132715,
                end_byte: 134115,
                slice_sha256: "3d0445e29f9bdff71a0501f0a5fff30a15073ae3955a0519708d97324c004f20",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/web",
        version: RC9,
        export: "httpHeader",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 18. `httpHeader` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.js",
                file_sha256: "32083d72a93231d8919bc78c57dea48d2c13bac8b571f07900c1b15fe95edfbf",
                start_byte: 76479,
                end_byte: 76526,
                slice_sha256: "cd98272f0f010dfe4990526fc6ba5a3508aea05b6f10233e5747ecd803b76e42",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 18. `httpHeader` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.dev.js",
                file_sha256: "bcbe02189e31a7ce29852ef74b9a12e7ad135d92a269cb89057d112e8075129d",
                start_byte: 84764,
                end_byte: 84811,
                slice_sha256: "cd98272f0f010dfe4990526fc6ba5a3508aea05b6f10233e5747ecd803b76e42",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 18. `httpHeader` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.observe.js",
                file_sha256: "b5b2950b906244f10ed54fc11a45076bcabd341f77e86249cb2ce462e72d8d74",
                start_byte: 77443,
                end_byte: 77490,
                slice_sha256: "cd98272f0f010dfe4990526fc6ba5a3508aea05b6f10233e5747ecd803b76e42",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 18. `httpHeader` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.js",
                file_sha256: "b96fc8399d4c9909fb8a275e76abde586c843d006dcb427fe4a56620bfee2c40",
                start_byte: 129688,
                end_byte: 131088,
                slice_sha256: "3d0445e29f9bdff71a0501f0a5fff30a15073ae3955a0519708d97324c004f20",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 18. `httpHeader` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.dev.js",
                file_sha256: "4636ed7864cad6f1b541d70872f51d1d98c35bb21bb30549a2b0c091570465c6",
                start_byte: 140686,
                end_byte: 142086,
                slice_sha256: "3d0445e29f9bdff71a0501f0a5fff30a15073ae3955a0519708d97324c004f20",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 18. `httpHeader` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.observe.js",
                file_sha256: "9919deaaa0c6b73d1f760247581e351c2d0778fc18784653b11579c4e11fe3fa",
                start_byte: 132715,
                end_byte: 134115,
                slice_sha256: "3d0445e29f9bdff71a0501f0a5fff30a15073ae3955a0519708d97324c004f20",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/web",
        version: RC9,
        export: "httpStatus",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 19. `httpStatus` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.js",
                file_sha256: "32083d72a93231d8919bc78c57dea48d2c13bac8b571f07900c1b15fe95edfbf",
                start_byte: 76442,
                end_byte: 76478,
                slice_sha256: "9de7ebfe186dfaf587fcaf60460297678b12e25a81e7c1221a574094397c926b",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 19. `httpStatus` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.dev.js",
                file_sha256: "bcbe02189e31a7ce29852ef74b9a12e7ad135d92a269cb89057d112e8075129d",
                start_byte: 84727,
                end_byte: 84763,
                slice_sha256: "9de7ebfe186dfaf587fcaf60460297678b12e25a81e7c1221a574094397c926b",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 19. `httpStatus` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.observe.js",
                file_sha256: "b5b2950b906244f10ed54fc11a45076bcabd341f77e86249cb2ce462e72d8d74",
                start_byte: 77406,
                end_byte: 77442,
                slice_sha256: "9de7ebfe186dfaf587fcaf60460297678b12e25a81e7c1221a574094397c926b",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 19. `httpStatus` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.js",
                file_sha256: "b96fc8399d4c9909fb8a275e76abde586c843d006dcb427fe4a56620bfee2c40",
                start_byte: 128658,
                end_byte: 129687,
                slice_sha256: "56233798652c4cdbb935eecde9e930996c50b7a747a4da9b37e3ec4dc19183e7",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 19. `httpStatus` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.dev.js",
                file_sha256: "4636ed7864cad6f1b541d70872f51d1d98c35bb21bb30549a2b0c091570465c6",
                start_byte: 139656,
                end_byte: 140685,
                slice_sha256: "56233798652c4cdbb935eecde9e930996c50b7a747a4da9b37e3ec4dc19183e7",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 19. `httpStatus` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.observe.js",
                file_sha256: "9919deaaa0c6b73d1f760247581e351c2d0778fc18784653b11579c4e11fe3fa",
                start_byte: 131685,
                end_byte: 132714,
                slice_sha256: "56233798652c4cdbb935eecde9e930996c50b7a747a4da9b37e3ec4dc19183e7",
            },
        ],
    },
    NegativeClaimRow {
        package: "@solidjs/web",
        version: RC9,
        export: "httpStatus",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 20. `httpStatus` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.js",
                file_sha256: "32083d72a93231d8919bc78c57dea48d2c13bac8b571f07900c1b15fe95edfbf",
                start_byte: 76442,
                end_byte: 76478,
                slice_sha256: "9de7ebfe186dfaf587fcaf60460297678b12e25a81e7c1221a574094397c926b",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 20. `httpStatus` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.dev.js",
                file_sha256: "bcbe02189e31a7ce29852ef74b9a12e7ad135d92a269cb89057d112e8075129d",
                start_byte: 84727,
                end_byte: 84763,
                slice_sha256: "9de7ebfe186dfaf587fcaf60460297678b12e25a81e7c1221a574094397c926b",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 20. `httpStatus` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/web.observe.js",
                file_sha256: "b5b2950b906244f10ed54fc11a45076bcabd341f77e86249cb2ce462e72d8d74",
                start_byte: 77406,
                end_byte: 77442,
                slice_sha256: "9de7ebfe186dfaf587fcaf60460297678b12e25a81e7c1221a574094397c926b",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 20. `httpStatus` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.js",
                file_sha256: "b96fc8399d4c9909fb8a275e76abde586c843d006dcb427fe4a56620bfee2c40",
                start_byte: 128658,
                end_byte: 129687,
                slice_sha256: "56233798652c4cdbb935eecde9e930996c50b7a747a4da9b37e3ec4dc19183e7",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 20. `httpStatus` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.dev.js",
                file_sha256: "4636ed7864cad6f1b541d70872f51d1d98c35bb21bb30549a2b0c091570465c6",
                start_byte: 139656,
                end_byte: 140685,
                slice_sha256: "56233798652c4cdbb935eecde9e930996c50b7a747a4da9b37e3ec4dc19183e7",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 20. `httpStatus` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
                archive_path: "dist/server.observe.js",
                file_sha256: "9919deaaa0c6b73d1f760247581e351c2d0778fc18784653b11579c4e11fe3fa",
                start_byte: 131685,
                end_byte: 132714,
                slice_sha256: "56233798652c4cdbb935eecde9e930996c50b7a747a4da9b37e3ec4dc19183e7",
            },
        ],
    },
    NegativeClaimRow {
        package: "solid-js",
        version: RC3,
        export: "For",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-782061630d49ccfa837915324fb3915d5acaa9393175694c750d975e49e591c5",
            start_byte: 6598,
            end_byte: 12189,
        }],
    },
    NegativeClaimRow {
        package: "solid-js",
        version: RC3,
        export: "For",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-782061630d49ccfa837915324fb3915d5acaa9393175694c750d975e49e591c5",
            start_byte: 6598,
            end_byte: 12189,
        }],
    },
    // `Loading` `creates` is withdrawn (RC3_SHOW_LOADING_AUDIT § 3): the server
    // body's `ssrLoadingBoundary` calls `ctx.serialize(id, "$$f")` and
    // `ctx.registerFragment(id, ..)` when its children are pending.
    // `reads` on the same audited bytes as the `creates` row below.
    // Its only argument-path atoms are parameter-rooted, which
    // `semantic-model.md` § reads [Decision 2026-09-10] assigns to the
    // caller; the closure denies a read of a proxy this export owns.
    NegativeClaimRow {
        package: "solid-js",
        version: RC3,
        export: "Match",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-782061630d49ccfa837915324fb3915d5acaa9393175694c750d975e49e591c5",
            start_byte: 6598,
            end_byte: 12189,
        }],
    },
    NegativeClaimRow {
        package: "solid-js",
        version: RC3,
        export: "Match",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-782061630d49ccfa837915324fb3915d5acaa9393175694c750d975e49e591c5",
            start_byte: 6598,
            end_byte: 12189,
        }],
    },
    // `reads` on the same audited bytes as the `creates` row below.
    // Its only argument-path atoms are parameter-rooted, which
    // `semantic-model.md` § reads [Decision 2026-09-10] assigns to the
    // caller; the closure denies a read of a proxy this export owns.
    NegativeClaimRow {
        package: "solid-js",
        version: RC3,
        export: "Repeat",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-782061630d49ccfa837915324fb3915d5acaa9393175694c750d975e49e591c5",
            start_byte: 6598,
            end_byte: 12189,
        }],
    },
    NegativeClaimRow {
        package: "solid-js",
        version: RC3,
        export: "Repeat",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-782061630d49ccfa837915324fb3915d5acaa9393175694c750d975e49e591c5",
            start_byte: 6598,
            end_byte: 12189,
        }],
    },
    // `Show` carries no row (RC3_SHOW_LOADING_AUDIT): `reads` is withdrawn
    // because its memos compute on the call's own stack and read
    // `conditionValue()` and `condition()`, memos `Show` created (§ 1);
    // `creates` because the server body's own `createMemo(() => props.when)`
    // reaches `ctx.serialize(id, deferred.promise, ..)` for a thenable `when`
    // (§ 2).
    // `reads` on the same audited bytes as the `creates` row below.
    // Its only argument-path atoms are parameter-rooted, which
    // `semantic-model.md` § reads [Decision 2026-09-10] assigns to the
    // caller; the closure denies a read of a proxy this export owns.
    NegativeClaimRow {
        package: "solid-js",
        version: RC3,
        export: "affects",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-92efacc141c683cc0a3779fa9106ff29f624ed876f3f6d0e0635643dec1d46fd",
            start_byte: 22614,
            end_byte: 24478,
        }],
    },
    NegativeClaimRow {
        package: "solid-js",
        version: RC3,
        export: "affects",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-92efacc141c683cc0a3779fa9106ff29f624ed876f3f6d0e0635643dec1d46fd",
            start_byte: 22614,
            end_byte: 24478,
        }],
    },
    // Implementation-audited, RC3_OWNER_CONTEXT_AUDIT § 2: a fresh symbol and a
    // provider function it defines but does not call, in all six bundles.
    NegativeClaimRow {
        package: "solid-js",
        version: RC3,
        export: "createContext",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC3_OWNER_CONTEXT_AUDIT,
                section: "## 2. `createContext` — archive `solid-js@2.0.0-rc.3`",
                archive_path: "dist/solid.js",
                file_sha256: "14af2d696eb0669c64973874601f691737aa1df359fced6dec55a523f34cfa1b",
                start_byte: 1241,
                end_byte: 1578,
                slice_sha256: "87cfe46a740a3e2a7e4a35a9df518de40c49d5156a36c310f20a29f120722829",
            },
            AuditedCitation::Implementation {
                audit: RC3_OWNER_CONTEXT_AUDIT,
                section: "## 2. `createContext` — archive `solid-js@2.0.0-rc.3`",
                archive_path: "dist/dev.js",
                file_sha256: "dfc362391cbc0b069cef8b8d0d72c99d34310231a76fd66ef615533424d3ac18",
                start_byte: 1248,
                end_byte: 1585,
                slice_sha256: "87cfe46a740a3e2a7e4a35a9df518de40c49d5156a36c310f20a29f120722829",
            },
            AuditedCitation::Implementation {
                audit: RC3_OWNER_CONTEXT_AUDIT,
                section: "## 2. `createContext` — archive `solid-js@2.0.0-rc.3`",
                archive_path: "dist/server.js",
                file_sha256: "63269da73b61b71fd775ef811f8ab88417c6ea6dda2de1e6f3c10d86b66fc8a8",
                start_byte: 44900,
                end_byte: 45237,
                slice_sha256: "87cfe46a740a3e2a7e4a35a9df518de40c49d5156a36c310f20a29f120722829",
            },
            AuditedCitation::Implementation {
                audit: RC3_OWNER_CONTEXT_AUDIT,
                section: "## 2. `createContext` — archive `solid-js@2.0.0-rc.3`",
                archive_path: "dist/solid.cjs",
                file_sha256: "d155966bc29d2bf46e3cb32c8839d885933a50b60c28c6d8abd70a7ac1147333",
                start_byte: 109,
                end_byte: 462,
                slice_sha256: "2d55c9abfe575798ed84748788bf721c824dabaab85da442beb709bf0a4aa22b",
            },
            AuditedCitation::Implementation {
                audit: RC3_OWNER_CONTEXT_AUDIT,
                section: "## 2. `createContext` — archive `solid-js@2.0.0-rc.3`",
                archive_path: "dist/dev.cjs",
                file_sha256: "4ca1b958df30ef4b0fa9cfd17206293073846d33147ad164208805dafde22e51",
                start_byte: 102,
                end_byte: 455,
                slice_sha256: "2d55c9abfe575798ed84748788bf721c824dabaab85da442beb709bf0a4aa22b",
            },
            AuditedCitation::Implementation {
                audit: RC3_OWNER_CONTEXT_AUDIT,
                section: "## 2. `createContext` — archive `solid-js@2.0.0-rc.3`",
                archive_path: "dist/server.cjs",
                file_sha256: "2e2ed5833323d48a43454b89f03de9f31346a3549a9934d8e67b3b9fe4f231a7",
                start_byte: 44856,
                end_byte: 45193,
                slice_sha256: "87cfe46a740a3e2a7e4a35a9df518de40c49d5156a36c310f20a29f120722829",
            },
        ],
    },
    // Implementation-audited, RC3_CORE_PRIMITIVES_AUDIT § 7.3, and **scoped**:
    // the flat row stays withheld (§ 7.4 — the `node`/`worker`/`deno` body's
    // derived overload reaches `ctx.serialize`, a create), so this row answers
    // only for a certification whose requested set carries `browser` and whose
    // `.` resolves to `dist/solid.js` under exactly that set.
    //
    // `dist/solid.js` alone, because it is the one bundle the section walked
    // end to end: every row of § 7.3's transitive table cites `solid.js` line
    // numbers. `dist/dev.js`, `dist/solid.cjs` and `dist/dev.cjs` were read at
    // the three-line wrapper only ("identical"), not through
    // `hydratedCreateSignal`, `hydrateSignalLike`, `subFetch` and the other
    // helpers the wrapper reaches, so a `development` or `require` resolution
    // refuses rather than inherit a reading of another file.
    //
    // The delegates are the two calls that table follows into
    // `@solidjs/signals` and dispositions there as "archive": `createSignal$1`
    // (R5, § 7.2) and `getOwner` (the `peekNextChildId(getOwner())` row, § 4).
    // The two non-primitive signals helpers it also reaches —
    // `markSnapshotScope` and `peekNextChildId` — rest on the archive-wide
    // host-boundary census (rc.3 § 1.5; rc.6 re-audit group A § 0.3), which
    // no row can name.
    NegativeClaimRow {
        package: "solid-js",
        version: RC3,
        export: "createSignal",
        domain: CallClaimDomain::Creates,
        scope: RowScope::HostTarget(HostTargetScope {
            condition: HostTargetCondition::Browser,
            runtime: &["dist/solid.js"],
            delegates: &[
                ("@solidjs/signals", "createSignal", CallClaimDomain::Creates),
                ("@solidjs/signals", "getOwner", CallClaimDomain::Creates),
            ],
        }),
        citations: &[AuditedCitation::Implementation {
            audit: RC3_CORE_PRIMITIVES_AUDIT,
            section: "### 7.3 R6 — `solid-js`' own `createSignal`, browser conditions",
            archive_path: "dist/solid.js",
            file_sha256: "14af2d696eb0669c64973874601f691737aa1df359fced6dec55a523f34cfa1b",
            start_byte: 23266,
            end_byte: 23358,
            slice_sha256: "3a46e2707a2ee2e2a7263af229d893a6a10fd041b8f0a08cbc29360eb65bda49",
        }],
    },
    NegativeClaimRow {
        package: "solid-js",
        version: RC3,
        export: "isPending",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-d41bc9d7ea19dd2a6dae7d51199a8448e627d5b6ac99a2eeb20cb02c7799c724",
            start_byte: 24560,
            end_byte: 26351,
        }],
    },
    NegativeClaimRow {
        package: "solid-js",
        version: RC3,
        export: "latest",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-13d78920672aa68cb9fb09d4b51dafaf4281d67628d73e4ba93f06de39512c40",
            start_byte: 2366,
            end_byte: 4224,
        }],
    },
    // `reads` on the same audited bytes as the `creates` row below.
    // Its only argument-path atoms are parameter-rooted, which
    // `semantic-model.md` § reads [Decision 2026-09-10] assigns to the
    // caller; the closure denies a read of a proxy this export owns.
    NegativeClaimRow {
        package: "solid-js",
        version: RC3,
        export: "refresh",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-49a501fb15bcd7c7961085bd54009503618b3729e6e49e8d299f0eb90ad9d322",
            start_byte: 4306,
            end_byte: 6516,
        }],
    },
    NegativeClaimRow {
        package: "solid-js",
        version: RC3,
        export: "refresh",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-49a501fb15bcd7c7961085bd54009503618b3729e6e49e8d299f0eb90ad9d322",
            start_byte: 4306,
            end_byte: 6516,
        }],
    },
    // Implementation-audited, RC3_OWNER_CONTEXT_AUDIT § 3: a context-map read
    // that may construct and throw an error, in all six bundles.
    NegativeClaimRow {
        package: "solid-js",
        version: RC3,
        export: "useContext",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC3_OWNER_CONTEXT_AUDIT,
                section: "## 3. `useContext` — archive `solid-js@2.0.0-rc.3`",
                archive_path: "dist/solid.js",
                file_sha256: "14af2d696eb0669c64973874601f691737aa1df359fced6dec55a523f34cfa1b",
                start_byte: 1579,
                end_byte: 1641,
                slice_sha256: "03f0fc70f94b38bb7a4b6720e59c06f7cf491dc297ffe4e5918960d5b9621618",
            },
            AuditedCitation::Implementation {
                audit: RC3_OWNER_CONTEXT_AUDIT,
                section: "## 3. `useContext` — archive `solid-js@2.0.0-rc.3`",
                archive_path: "dist/dev.js",
                file_sha256: "dfc362391cbc0b069cef8b8d0d72c99d34310231a76fd66ef615533424d3ac18",
                start_byte: 1586,
                end_byte: 1648,
                slice_sha256: "03f0fc70f94b38bb7a4b6720e59c06f7cf491dc297ffe4e5918960d5b9621618",
            },
            AuditedCitation::Implementation {
                audit: RC3_OWNER_CONTEXT_AUDIT,
                section: "## 3. `useContext` — archive `solid-js@2.0.0-rc.3`",
                archive_path: "dist/server.js",
                file_sha256: "63269da73b61b71fd775ef811f8ab88417c6ea6dda2de1e6f3c10d86b66fc8a8",
                start_byte: 45238,
                end_byte: 45494,
                slice_sha256: "3cbc46d0ef0841dd0dab0674ca61438056a050d6d118d44c094404c42ad8dd5f",
            },
            AuditedCitation::Implementation {
                audit: RC3_OWNER_CONTEXT_AUDIT,
                section: "## 3. `useContext` — archive `solid-js@2.0.0-rc.3`",
                archive_path: "dist/solid.cjs",
                file_sha256: "d155966bc29d2bf46e3cb32c8839d885933a50b60c28c6d8abd70a7ac1147333",
                start_byte: 463,
                end_byte: 533,
                slice_sha256: "03c1ef4583a6dcb0e5658b2966a14b3fc665de4d2036521d7aab45e12ac87d54",
            },
            AuditedCitation::Implementation {
                audit: RC3_OWNER_CONTEXT_AUDIT,
                section: "## 3. `useContext` — archive `solid-js@2.0.0-rc.3`",
                archive_path: "dist/dev.cjs",
                file_sha256: "4ca1b958df30ef4b0fa9cfd17206293073846d33147ad164208805dafde22e51",
                start_byte: 456,
                end_byte: 526,
                slice_sha256: "03c1ef4583a6dcb0e5658b2966a14b3fc665de4d2036521d7aab45e12ac87d54",
            },
            AuditedCitation::Implementation {
                audit: RC3_OWNER_CONTEXT_AUDIT,
                section: "## 3. `useContext` — archive `solid-js@2.0.0-rc.3`",
                archive_path: "dist/server.cjs",
                file_sha256: "2e2ed5833323d48a43454b89f03de9f31346a3549a9934d8e67b3b9fe4f231a7",
                start_byte: 45194,
                end_byte: 45458,
                slice_sha256: "6df1017268a90bca0b6774692a30267d70cbab07bfd698afa25928116cbd71fb",
            },
        ],
    },
    // `solid-js@2.0.0-rc.9`, read on its own bytes (RC9_CORE_WEB_AUDIT) in all six
    // builds its `.` entry can select, and every row cites all six; none rests on
    // the rc.3 bundled document. `For` reads only parameter-rooted props and hands
    // `mapArray` export-authored thunks over them; its internal memo is read only
    // by a later invocation of the returned `list` (§ 1).
    NegativeClaimRow {
        package: "solid-js",
        version: RC9,
        export: "For",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 1. `For` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.js",
                file_sha256: "0238f90858359bc71da523cae1c38dc769f6d86502ae37bcf1c7d72cc977794d",
                start_byte: 37042,
                end_byte: 37467,
                slice_sha256: "3f85ecafbe4729ce5c20deac5622cf43aed0153bf0154f5b1fa27513f3aeb065",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 1. `For` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.dev.js",
                file_sha256: "7baf8808f6bd87011707621ab9137a41416a585db3d12c89f7a7efcdec2411de",
                start_byte: 39271,
                end_byte: 39722,
                slice_sha256: "eabb480fb722cb1bf1aec173113ad7c3b152476d54c436dd6b04f37ff59441b5",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 1. `For` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.observe.js",
                file_sha256: "c337cd4b1b90e5dda31dccd4373d7d72e3aec07a7d40e1a6fefd903b63b16ffb",
                start_byte: 37372,
                end_byte: 37823,
                slice_sha256: "eabb480fb722cb1bf1aec173113ad7c3b152476d54c436dd6b04f37ff59441b5",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 1. `For` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.js",
                file_sha256: "9c25fe06f9aab7657bcd87c7ad4b12ac413ebab3f02bae651da253a7478db7f9",
                start_byte: 65074,
                end_byte: 65296,
                slice_sha256: "36dc66a015203248b9aa9eef6309f2386d8a3b74406d85a925cd123e0d2da223",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 1. `For` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.dev.js",
                file_sha256: "129cbe735cceed1ff627aaab146e3c7c5e2032ed0a1f980134334226651ab7e7",
                start_byte: 74377,
                end_byte: 74599,
                slice_sha256: "36dc66a015203248b9aa9eef6309f2386d8a3b74406d85a925cd123e0d2da223",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 1. `For` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.observe.js",
                file_sha256: "0d1519f0613584c577288aa2519c29fb42c5aa1446742f84f3559841ec22aede",
                start_byte: 69358,
                end_byte: 69580,
                slice_sha256: "36dc66a015203248b9aa9eef6309f2386d8a3b74406d85a925cd123e0d2da223",
            },
        ],
    },
    NegativeClaimRow {
        package: "solid-js",
        version: RC9,
        export: "For",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 2. `For` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.js",
                file_sha256: "0238f90858359bc71da523cae1c38dc769f6d86502ae37bcf1c7d72cc977794d",
                start_byte: 37042,
                end_byte: 37467,
                slice_sha256: "3f85ecafbe4729ce5c20deac5622cf43aed0153bf0154f5b1fa27513f3aeb065",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 2. `For` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.dev.js",
                file_sha256: "7baf8808f6bd87011707621ab9137a41416a585db3d12c89f7a7efcdec2411de",
                start_byte: 39271,
                end_byte: 39722,
                slice_sha256: "eabb480fb722cb1bf1aec173113ad7c3b152476d54c436dd6b04f37ff59441b5",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 2. `For` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.observe.js",
                file_sha256: "c337cd4b1b90e5dda31dccd4373d7d72e3aec07a7d40e1a6fefd903b63b16ffb",
                start_byte: 37372,
                end_byte: 37823,
                slice_sha256: "eabb480fb722cb1bf1aec173113ad7c3b152476d54c436dd6b04f37ff59441b5",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 2. `For` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.js",
                file_sha256: "9c25fe06f9aab7657bcd87c7ad4b12ac413ebab3f02bae651da253a7478db7f9",
                start_byte: 65074,
                end_byte: 65296,
                slice_sha256: "36dc66a015203248b9aa9eef6309f2386d8a3b74406d85a925cd123e0d2da223",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 2. `For` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.dev.js",
                file_sha256: "129cbe735cceed1ff627aaab146e3c7c5e2032ed0a1f980134334226651ab7e7",
                start_byte: 74377,
                end_byte: 74599,
                slice_sha256: "36dc66a015203248b9aa9eef6309f2386d8a3b74406d85a925cd123e0d2da223",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 2. `For` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.observe.js",
                file_sha256: "0d1519f0613584c577288aa2519c29fb42c5aa1446742f84f3559841ec22aede",
                start_byte: 69358,
                end_byte: 69580,
                slice_sha256: "36dc66a015203248b9aa9eef6309f2386d8a3b74406d85a925cd123e0d2da223",
            },
        ],
    },
    // `return props` in every build: no call, no property access (§ 5).
    NegativeClaimRow {
        package: "solid-js",
        version: RC9,
        export: "Match",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 5. `Match` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.js",
                file_sha256: "0238f90858359bc71da523cae1c38dc769f6d86502ae37bcf1c7d72cc977794d",
                start_byte: 39758,
                end_byte: 39799,
                slice_sha256: "54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 5. `Match` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.dev.js",
                file_sha256: "7baf8808f6bd87011707621ab9137a41416a585db3d12c89f7a7efcdec2411de",
                start_byte: 42212,
                end_byte: 42253,
                slice_sha256: "54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 5. `Match` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.observe.js",
                file_sha256: "c337cd4b1b90e5dda31dccd4373d7d72e3aec07a7d40e1a6fefd903b63b16ffb",
                start_byte: 40303,
                end_byte: 40344,
                slice_sha256: "54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 5. `Match` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.js",
                file_sha256: "9c25fe06f9aab7657bcd87c7ad4b12ac413ebab3f02bae651da253a7478db7f9",
                start_byte: 67106,
                end_byte: 67147,
                slice_sha256: "54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 5. `Match` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.dev.js",
                file_sha256: "129cbe735cceed1ff627aaab146e3c7c5e2032ed0a1f980134334226651ab7e7",
                start_byte: 76409,
                end_byte: 76450,
                slice_sha256: "54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 5. `Match` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.observe.js",
                file_sha256: "0d1519f0613584c577288aa2519c29fb42c5aa1446742f84f3559841ec22aede",
                start_byte: 71390,
                end_byte: 71431,
                slice_sha256: "54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f",
            },
        ],
    },
    NegativeClaimRow {
        package: "solid-js",
        version: RC9,
        export: "Match",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 6. `Match` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.js",
                file_sha256: "0238f90858359bc71da523cae1c38dc769f6d86502ae37bcf1c7d72cc977794d",
                start_byte: 39758,
                end_byte: 39799,
                slice_sha256: "54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 6. `Match` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.dev.js",
                file_sha256: "7baf8808f6bd87011707621ab9137a41416a585db3d12c89f7a7efcdec2411de",
                start_byte: 42212,
                end_byte: 42253,
                slice_sha256: "54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 6. `Match` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.observe.js",
                file_sha256: "c337cd4b1b90e5dda31dccd4373d7d72e3aec07a7d40e1a6fefd903b63b16ffb",
                start_byte: 40303,
                end_byte: 40344,
                slice_sha256: "54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 6. `Match` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.js",
                file_sha256: "9c25fe06f9aab7657bcd87c7ad4b12ac413ebab3f02bae651da253a7478db7f9",
                start_byte: 67106,
                end_byte: 67147,
                slice_sha256: "54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 6. `Match` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.dev.js",
                file_sha256: "129cbe735cceed1ff627aaab146e3c7c5e2032ed0a1f980134334226651ab7e7",
                start_byte: 76409,
                end_byte: 76450,
                slice_sha256: "54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 6. `Match` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.observe.js",
                file_sha256: "0d1519f0613584c577288aa2519c29fb42c5aa1446742f84f3559841ec22aede",
                start_byte: 71390,
                end_byte: 71431,
                slice_sha256: "54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f",
            },
        ],
    },
    NegativeClaimRow {
        package: "solid-js",
        version: RC9,
        export: "Repeat",
        domain: CallClaimDomain::Reads,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 3. `Repeat` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.js",
                file_sha256: "0238f90858359bc71da523cae1c38dc769f6d86502ae37bcf1c7d72cc977794d",
                start_byte: 37468,
                end_byte: 37749,
                slice_sha256: "540fa67ec55df13ad8823dae14825bbf275c5c3697d4d3b8b3f516c8aa14d66a",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 3. `Repeat` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.dev.js",
                file_sha256: "7baf8808f6bd87011707621ab9137a41416a585db3d12c89f7a7efcdec2411de",
                start_byte: 39723,
                end_byte: 40033,
                slice_sha256: "c9b26a452e05bdb29c555e798b72a155d0eeee2c0d21d3df3025cd70647a1a95",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 3. `Repeat` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.observe.js",
                file_sha256: "c337cd4b1b90e5dda31dccd4373d7d72e3aec07a7d40e1a6fefd903b63b16ffb",
                start_byte: 37824,
                end_byte: 38134,
                slice_sha256: "c9b26a452e05bdb29c555e798b72a155d0eeee2c0d21d3df3025cd70647a1a95",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 3. `Repeat` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.js",
                file_sha256: "9c25fe06f9aab7657bcd87c7ad4b12ac413ebab3f02bae651da253a7478db7f9",
                start_byte: 65297,
                end_byte: 65578,
                slice_sha256: "540fa67ec55df13ad8823dae14825bbf275c5c3697d4d3b8b3f516c8aa14d66a",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 3. `Repeat` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.dev.js",
                file_sha256: "129cbe735cceed1ff627aaab146e3c7c5e2032ed0a1f980134334226651ab7e7",
                start_byte: 74600,
                end_byte: 74881,
                slice_sha256: "540fa67ec55df13ad8823dae14825bbf275c5c3697d4d3b8b3f516c8aa14d66a",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 3. `Repeat` — `reads` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.observe.js",
                file_sha256: "0d1519f0613584c577288aa2519c29fb42c5aa1446742f84f3559841ec22aede",
                start_byte: 69581,
                end_byte: 69862,
                slice_sha256: "540fa67ec55df13ad8823dae14825bbf275c5c3697d4d3b8b3f516c8aa14d66a",
            },
        ],
    },
    NegativeClaimRow {
        package: "solid-js",
        version: RC9,
        export: "Repeat",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 4. `Repeat` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.js",
                file_sha256: "0238f90858359bc71da523cae1c38dc769f6d86502ae37bcf1c7d72cc977794d",
                start_byte: 37468,
                end_byte: 37749,
                slice_sha256: "540fa67ec55df13ad8823dae14825bbf275c5c3697d4d3b8b3f516c8aa14d66a",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 4. `Repeat` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.dev.js",
                file_sha256: "7baf8808f6bd87011707621ab9137a41416a585db3d12c89f7a7efcdec2411de",
                start_byte: 39723,
                end_byte: 40033,
                slice_sha256: "c9b26a452e05bdb29c555e798b72a155d0eeee2c0d21d3df3025cd70647a1a95",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 4. `Repeat` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.observe.js",
                file_sha256: "c337cd4b1b90e5dda31dccd4373d7d72e3aec07a7d40e1a6fefd903b63b16ffb",
                start_byte: 37824,
                end_byte: 38134,
                slice_sha256: "c9b26a452e05bdb29c555e798b72a155d0eeee2c0d21d3df3025cd70647a1a95",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 4. `Repeat` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.js",
                file_sha256: "9c25fe06f9aab7657bcd87c7ad4b12ac413ebab3f02bae651da253a7478db7f9",
                start_byte: 65297,
                end_byte: 65578,
                slice_sha256: "540fa67ec55df13ad8823dae14825bbf275c5c3697d4d3b8b3f516c8aa14d66a",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 4. `Repeat` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.dev.js",
                file_sha256: "129cbe735cceed1ff627aaab146e3c7c5e2032ed0a1f980134334226651ab7e7",
                start_byte: 74600,
                end_byte: 74881,
                slice_sha256: "540fa67ec55df13ad8823dae14825bbf275c5c3697d4d3b8b3f516c8aa14d66a",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 4. `Repeat` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.observe.js",
                file_sha256: "0d1519f0613584c577288aa2519c29fb42c5aa1446742f84f3559841ec22aede",
                start_byte: 69581,
                end_byte: 69862,
                slice_sha256: "540fa67ec55df13ad8823dae14825bbf275c5c3697d4d3b8b3f516c8aa14d66a",
            },
        ],
    },
    // A fresh symbol and a provider it defines but does not call (§ 10).
    NegativeClaimRow {
        package: "solid-js",
        version: RC9,
        export: "createContext",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 10. `createContext` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.js",
                file_sha256: "0238f90858359bc71da523cae1c38dc769f6d86502ae37bcf1c7d72cc977794d",
                start_byte: 1295,
                end_byte: 1634,
                slice_sha256: "c3be410a00e4ddc5fd5150138223a341e6280d8489dc94d2e6ac6560665d899f",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 10. `createContext` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.dev.js",
                file_sha256: "7baf8808f6bd87011707621ab9137a41416a585db3d12c89f7a7efcdec2411de",
                start_byte: 1324,
                end_byte: 1663,
                slice_sha256: "c3be410a00e4ddc5fd5150138223a341e6280d8489dc94d2e6ac6560665d899f",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 10. `createContext` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.observe.js",
                file_sha256: "c337cd4b1b90e5dda31dccd4373d7d72e3aec07a7d40e1a6fefd903b63b16ffb",
                start_byte: 1317,
                end_byte: 1656,
                slice_sha256: "c3be410a00e4ddc5fd5150138223a341e6280d8489dc94d2e6ac6560665d899f",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 10. `createContext` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.js",
                file_sha256: "9c25fe06f9aab7657bcd87c7ad4b12ac413ebab3f02bae651da253a7478db7f9",
                start_byte: 49988,
                end_byte: 50325,
                slice_sha256: "87cfe46a740a3e2a7e4a35a9df518de40c49d5156a36c310f20a29f120722829",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 10. `createContext` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.dev.js",
                file_sha256: "129cbe735cceed1ff627aaab146e3c7c5e2032ed0a1f980134334226651ab7e7",
                start_byte: 54369,
                end_byte: 54706,
                slice_sha256: "87cfe46a740a3e2a7e4a35a9df518de40c49d5156a36c310f20a29f120722829",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 10. `createContext` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.observe.js",
                file_sha256: "0d1519f0613584c577288aa2519c29fb42c5aa1446742f84f3559841ec22aede",
                start_byte: 52107,
                end_byte: 52444,
                slice_sha256: "87cfe46a740a3e2a7e4a35a9df518de40c49d5156a36c310f20a29f120722829",
            },
        ],
    },
    // RC9_MERGE_OMIT_MEMO_AUDIT § 4, **scoped** like `createSignal` below: the
    // flat row is withheld (§ 3, the server `createMemo` hands a thenable
    // result to `processResult`, which reaches `ctx.serialize`). The browser
    // wrapper and `hydratedCreateMemo` reach exactly the 27-definition closure
    // RC9_CORE_WEB_AUDIT § 13 walked from `createSignal`, byte-identical in all
    // three builds `browser` can select. The delegates are every canonical
    // `@solidjs/signals` call that closure makes: `createMemo$1` (the wrapper,
    // and `coreFn` through `hydrateSignalLike`), `createSignal$1`
    // (`withHydrationGate`, `armLiveTakeover`) and `getOwner`.
    NegativeClaimRow {
        package: "solid-js",
        version: RC9,
        export: "createMemo",
        domain: CallClaimDomain::Creates,
        scope: RowScope::HostTarget(HostTargetScope {
            condition: HostTargetCondition::Browser,
            runtime: &[
                "dist/solid.dev.js",
                "dist/solid.js",
                "dist/solid.observe.js",
            ],
            delegates: &[
                ("@solidjs/signals", "createMemo", CallClaimDomain::Creates),
                ("@solidjs/signals", "createSignal", CallClaimDomain::Creates),
                ("@solidjs/signals", "getOwner", CallClaimDomain::Creates),
            ],
        }),
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_MERGE_OMIT_MEMO_AUDIT,
                section: "## 4. `createMemo` — `creates`, `browser` host target — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.js",
                file_sha256: "0238f90858359bc71da523cae1c38dc769f6d86502ae37bcf1c7d72cc977794d",
                start_byte: 23820,
                end_byte: 23905,
                slice_sha256: "1528dcd8aeb175ac5dda5fa0477fdbe76d6a7498ef43ce7a0ab6ffc196f982f5",
            },
            AuditedCitation::Implementation {
                audit: RC9_MERGE_OMIT_MEMO_AUDIT,
                section: "## 4. `createMemo` — `creates`, `browser` host target — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.dev.js",
                file_sha256: "7baf8808f6bd87011707621ab9137a41416a585db3d12c89f7a7efcdec2411de",
                start_byte: 25516,
                end_byte: 25601,
                slice_sha256: "1528dcd8aeb175ac5dda5fa0477fdbe76d6a7498ef43ce7a0ab6ffc196f982f5",
            },
            AuditedCitation::Implementation {
                audit: RC9_MERGE_OMIT_MEMO_AUDIT,
                section: "## 4. `createMemo` — `creates`, `browser` host target — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.observe.js",
                file_sha256: "c337cd4b1b90e5dda31dccd4373d7d72e3aec07a7d40e1a6fefd903b63b16ffb",
                start_byte: 24140,
                end_byte: 24225,
                slice_sha256: "1528dcd8aeb175ac5dda5fa0477fdbe76d6a7498ef43ce7a0ab6ffc196f982f5",
            },
        ],
    },
    // RC9_CORE_WEB_AUDIT § 13, and **scoped** like its rc.3 twin: the flat row
    // is withheld (§ 12, the server's derived overload reaches `ctx.serialize`).
    // All three builds the `browser` condition can select are cited, because
    // the walk found their closures byte-identical definition for definition.
    // The delegates are the two canonical `@solidjs/signals` calls the walk
    // reaches; `solid-js@2.0.0-rc.9` depends on `@solidjs/signals@^2.0.0-rc.9`,
    // whose own `createSignal` and `getOwner` `creates` rows (RC9_PARITY_AUDIT,
    // RC9_SIGNALS_AUDIT) answer both. The non-primitive signals reaches
    // -- `markSnapshotScope`, `peekNextChildId`, and the bound `read`/`setSignal`
    // of the gate and live signals -- rest on the rc.9 signals archive census.
    NegativeClaimRow {
        package: "solid-js",
        version: RC9,
        export: "createSignal",
        domain: CallClaimDomain::Creates,
        scope: RowScope::HostTarget(HostTargetScope {
            condition: HostTargetCondition::Browser,
            runtime: &[
                "dist/solid.dev.js",
                "dist/solid.js",
                "dist/solid.observe.js",
            ],
            delegates: &[
                ("@solidjs/signals", "createSignal", CallClaimDomain::Creates),
                ("@solidjs/signals", "getOwner", CallClaimDomain::Creates),
            ],
        }),
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 13. `createSignal` — `creates`, `browser` host target — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.js",
                file_sha256: "0238f90858359bc71da523cae1c38dc769f6d86502ae37bcf1c7d72cc977794d",
                start_byte: 23906,
                end_byte: 23997,
                slice_sha256: "96473897517769f0074a40f4e1657dfb1d580a915549f978b7651ab38f3ec8a7",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 13. `createSignal` — `creates`, `browser` host target — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.dev.js",
                file_sha256: "7baf8808f6bd87011707621ab9137a41416a585db3d12c89f7a7efcdec2411de",
                start_byte: 25602,
                end_byte: 25693,
                slice_sha256: "96473897517769f0074a40f4e1657dfb1d580a915549f978b7651ab38f3ec8a7",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 13. `createSignal` — `creates`, `browser` host target — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.observe.js",
                file_sha256: "c337cd4b1b90e5dda31dccd4373d7d72e3aec07a7d40e1a6fefd903b63b16ffb",
                start_byte: 24226,
                end_byte: 24317,
                slice_sha256: "96473897517769f0074a40f4e1657dfb1d580a915549f978b7651ab38f3ec8a7",
            },
        ],
    },
    // A context-map read that may construct and throw an error (§ 11).
    NegativeClaimRow {
        package: "solid-js",
        version: RC9,
        export: "useContext",
        domain: CallClaimDomain::Creates,
        scope: RowScope::EveryCondition,
        citations: &[
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 11. `useContext` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.js",
                file_sha256: "0238f90858359bc71da523cae1c38dc769f6d86502ae37bcf1c7d72cc977794d",
                start_byte: 1635,
                end_byte: 1697,
                slice_sha256: "03f0fc70f94b38bb7a4b6720e59c06f7cf491dc297ffe4e5918960d5b9621618",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 11. `useContext` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.dev.js",
                file_sha256: "7baf8808f6bd87011707621ab9137a41416a585db3d12c89f7a7efcdec2411de",
                start_byte: 1664,
                end_byte: 1726,
                slice_sha256: "03f0fc70f94b38bb7a4b6720e59c06f7cf491dc297ffe4e5918960d5b9621618",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 11. `useContext` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/solid.observe.js",
                file_sha256: "c337cd4b1b90e5dda31dccd4373d7d72e3aec07a7d40e1a6fefd903b63b16ffb",
                start_byte: 1657,
                end_byte: 1719,
                slice_sha256: "03f0fc70f94b38bb7a4b6720e59c06f7cf491dc297ffe4e5918960d5b9621618",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 11. `useContext` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.js",
                file_sha256: "9c25fe06f9aab7657bcd87c7ad4b12ac413ebab3f02bae651da253a7478db7f9",
                start_byte: 50326,
                end_byte: 50582,
                slice_sha256: "3cbc46d0ef0841dd0dab0674ca61438056a050d6d118d44c094404c42ad8dd5f",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 11. `useContext` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.dev.js",
                file_sha256: "129cbe735cceed1ff627aaab146e3c7c5e2032ed0a1f980134334226651ab7e7",
                start_byte: 54707,
                end_byte: 54963,
                slice_sha256: "3cbc46d0ef0841dd0dab0674ca61438056a050d6d118d44c094404c42ad8dd5f",
            },
            AuditedCitation::Implementation {
                audit: RC9_CORE_WEB_AUDIT,
                section: "### 11. `useContext` — `creates` — archive `solid-js@2.0.0-rc.9`",
                archive_path: "dist/server.observe.js",
                file_sha256: "0d1519f0613584c577288aa2519c29fb42c5aa1446742f84f3559841ec22aede",
                start_byte: 52445,
                end_byte: 52701,
                slice_sha256: "3cbc46d0ef0841dd0dab0674ca61438056a050d6d118d44c094404c42ad8dd5f",
            },
        ],
    },
];

static NEGATIVE_AUTHORITY: DialectNegativeAuthority = DialectNegativeAuthority {
    archives: AUDITED_ARCHIVES,
    rows: NEGATIVE_ROWS,
};

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

    /// 2.0 splits the definitions across three archives: the reactive core
    /// lives in `@solidjs/signals`, the DOM primitives in `@solidjs/web`, and
    /// `solid-js` re-exports the core under the dialect's public spellings
    /// while declaring some of its own. The other `@solidjs/*` packages
    /// (`router`, `meta`, `start`, `element`) are consumers of these three and
    /// are deliberately absent.
    fn primitive_defining_packages(&self) -> &'static [&'static str] {
        &["solid-js", "@solidjs/signals", "@solidjs/web"]
    }

    fn ecosystem_scopes(&self) -> &'static [&'static str] {
        &["@solidjs/"]
    }

    /// ADR 0163: 2.0's dependency tracking lives in `@solidjs/signals`, whose
    /// every audited release (rc.3, rc.6, rc.9) exports these four names from
    /// `dist/prod/index.js` and `dist/dev.js`, and rc.9 also from
    /// `dist/observe/index.js`. A one-argument `createMemo` runs its compute
    /// when it is created, under the memo as observer, and a read inside it
    /// links a dependency onto the memo's node -- `link()` in
    /// `dist/prod/core/graph.js` (rc.9). The node's two dependency fields are
    /// named differently in every one of those seven builds (`nt`/`Ye`,
    /// `ut`/`je`, `Se`/`ot`, `ee`/`Fe`, and `_deps`/`_depsTail` in the dev
    /// builds), which is why the veto module calibrates them at run time and
    /// nothing here names one.
    fn tracking_runtime(&self) -> Option<&'static TrackingRuntime> {
        static SIGNALS: TrackingRuntime = TrackingRuntime {
            package: "@solidjs/signals",
            create_root: "createRoot",
            create_memo: "createMemo",
            create_signal: "createSignal",
            get_observer: "getObserver",
        };
        Some(&SIGNALS)
    }

    /// Reviewed Solid 2 semantics in this module, not a package certificate.
    /// Revision 2 (2026-09-26) models the three rc.9 callback forms: `until`,
    /// `dynamic`'s option-selected call forms, and `omit`'s predicate.
    /// Revision 3 (2026-09-26) answers the `omit` predicate per release
    /// (rc.3 has none) and makes code inside an rc.9 predicate uncertifiable.
    /// Revision 4 (2026-09-27) states that `createComponent` renders its first
    /// argument (`Solid2::renders_component_argument`, ADR 0136).
    /// Revision 5 (2026-09-28) states that `Dynamic` renders its `component`
    /// prop and that a one-argument `createMemo` hands its compute's results
    /// only to its accessor (`Solid2::component_prop_renderers`,
    /// `Solid2::accessor_yields_only_its_compute`, ADR 0138).
    fn runtime_model_identity(&self) -> &'static str {
        "solid-v2/model-5"
    }

    /// [`AUDITED_ARCHIVES`] and [`NEGATIVE_ROWS`]: archive-scoped rows read out
    /// of the audited rc.3 contract documents or, by hand, out of each
    /// archive's runtime bytes, with the withholdings named there. The counts
    /// per archive are pinned by `the_negative_table_is_derived_from_the_audited_documents`.
    fn negative_claim_authority(&self) -> &'static DialectNegativeAuthority {
        &NEGATIVE_AUTHORITY
    }

    fn primitive(&self, name: &str) -> Option<Primitive> {
        static INDEX: crate::NameIndex = crate::NameIndex::new();
        lookup(&INDEX, &[TABLE], name).filter(|primitive| self.exports(*primitive))
    }

    /// The two `dynamic` call forms have no row of their own in [`TABLE`]: a
    /// form is reached from the name, never the other way round, so both
    /// spell as the export they are a call of.
    fn name_of(&self, primitive: Primitive) -> Option<&'static str> {
        match primitive {
            Primitive::DynamicStatic | Primitive::DynamicUnknownForm => Some("dynamic"),
            _ if !self.exports(primitive) => None,
            _ => reverse(TABLE, primitive),
        }
    }

    /// `dynamic(source, options?)` is the one 2.0 export whose runtime is
    /// chosen by an option. `@solidjs/web@2.0.0-rc.9` opens every build of it
    /// with `if (options?.static) …` (`dist/web.dev.js:2199`,
    /// `dist/web.js:2034`, `dist/web.observe.js:2056`, `dist/server.js:3729`,
    /// `dist/server.dev.js:3977`, `dist/server.observe.js:3817`), a
    /// truthiness test, so:
    ///
    /// - a literal `static: true` is [`Primitive::DynamicStatic`];
    /// - no options, a `null`/`undefined` literal, an exact literal without
    ///   `static`, or `static: false` is the default runtime, unchanged: the
    ///   first statement is skipped and the lazy tracked memo is built exactly
    ///   as rc.3 builds it;
    /// - anything the syntax does not prove is
    ///   [`Primitive::DynamicUnknownForm`], which states nothing. Keeping the
    ///   default model there instead would publish `tracked` in a package
    ///   contract, and claim a created owner, for a source the runtime may run
    ///   once, untracked, under the caller's owner.
    ///
    /// Answered from the resolved `@solidjs/web` (`Solid2::dynamic_options`),
    /// because only rc.9's runtime reads the option. On rc.0-rc.8 every call
    /// is the default form, whatever it passes: rc.0-rc.6 declare
    /// `dynamic(source)` with one parameter (TS2554 on a second), and rc.7 and
    /// rc.8 accept `DynamicOptions { deferStream }` (TS2353 on `static`) that
    /// the client bundle never reads. Modelling rc.9's static form there would
    /// state rc.9 behaviour -- no owner, an untracked source -- about a runtime
    /// that creates the memo and its owner as always (the rc.1-rc.8 review
    /// § 6). On a web release nobody read, an option-bearing call is
    /// [`Primitive::DynamicUnknownForm`], which states nothing.
    fn call_form(
        &self,
        primitive: Primitive,
        option: &dyn Fn(usize, &str) -> crate::OptionLiteral,
    ) -> Primitive {
        use crate::OptionLiteral;
        if primitive != Primitive::Dynamic {
            return primitive;
        }
        match (self.dynamic_options, option(1, "static")) {
            (releases::DynamicOptions::Ignored, _)
            | (_, OptionLiteral::Absent | OptionLiteral::False) => Primitive::Dynamic,
            (releases::DynamicOptions::StaticForm, OptionLiteral::True) => Primitive::DynamicStatic,
            (releases::DynamicOptions::StaticForm, OptionLiteral::Unknown)
            | (releases::DynamicOptions::Unread, OptionLiteral::True | OptionLiteral::Unknown) => {
                Primitive::DynamicUnknownForm
            }
        }
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
            // Both `dynamic` call forms keep the source at 0; the form
            // changes how it runs, not where it sits.
            | Primitive::DynamicStatic
            | Primitive::DynamicUnknownForm
            | Primitive::ClientOnly
            | Primitive::Flush
            | Primitive::Untrack
            | Primitive::OnSettled
            | Primitive::CreateReaction
            | Primitive::Action
            // latest(fn), isPending(fn), resolve(fn), until(fn, options?)
            // each take one thunk.
            | Primitive::Latest
            | Primitive::IsPending
            | Primitive::Resolve
            | Primitive::Until
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
    ///
    /// `flush(fn)` is absent for the same reason. It runs `fn()` inline
    /// between a `syncDepth` increment and the drain and touches neither
    /// `context` nor the listener (`@solidjs/signals@2.0.0-rc.0`
    /// `dist/dev.js:1085-1099`, rc.3 `dist/dev.js:1788-1802`, rc.9
    /// `dist/dev-shared.js:2202-2230`), so its callback inherits the caller's
    /// read role exactly as [`Solid2::callback_preserves_owner_write_context`]
    /// already makes it inherit the caller's write role. Probed on every
    /// published rc.0-rc.9 client build, dev and prod: `createMemo(() =>
    /// flush(() => count()))` re-runs when `count` is written, as the bare
    /// read does and `untrack` does not; `getObserver()` inside `flush(fn)` is
    /// the memo's; an effect compute reading through `flush(fn)` re-runs; and
    /// `flush(() => count())` in a component body raises
    /// `STRICT_READ_UNTRACKED` (dev) exactly as `count()` there does.
    fn runs_callback_deferred(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateRoot
                | Primitive::CreateRevealOrder
                | Primitive::Untrack
                | Primitive::OnSettled
                | Primitive::CreateReaction
                | Primitive::Action
                | Primitive::ClientOnly
                | Primitive::RunWithOwner
                // resolve(fn) returns a Promise -- the thunk's reads settle
                // outside the current computation.
                | Primitive::Resolve
                // until(fn) is resolve's shape (see `callback_executions`):
                // its predicate is the compute of a user effect under a fresh
                // root, and the caller has no observer for it to subscribe.
                | Primitive::Until
                | Primitive::Lazy
                // dynamic(source, { static: true }) is
                // `staticDynamic(untrack(source))`: the signals `untrack`,
                // which clears the listener around the call.
                | Primitive::DynamicStatic
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
    ///
    /// `dynamic(source, { static: true })` runs its source through that same
    /// `untrack` (`@solidjs/web@2.0.0-rc.9` `dist/web.dev.js:2199`), with no
    /// owner of its own, so a write in a static source is exactly as legal as
    /// at the `dynamic` call.
    ///
    /// `flush(fn)` runs `fn()` inline between a `syncDepth` increment and the
    /// drain, touching neither `context` nor the listener
    /// (`@solidjs/signals@2.0.0-rc.0` `dist/dev.js:1085-1099`, rc.3
    /// `dist/dev.js:1788-1802`, rc.9 `dist/dev-shared.js:2202-2230`). Probed
    /// on every published rc.0-rc.9 dev client build: `getOwner()`
    /// inside `flush(fn)` is the caller's owner, and a signal setter inside
    /// `flush(fn)` throws `REACTIVE_WRITE_IN_OWNED_SCOPE` in a memo compute,
    /// a component body and a `createRoot` body, inline or passed by name,
    /// while the same call at module scope succeeds.
    fn callback_preserves_owner_write_context(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::Untrack | Primitive::DynamicStatic | Primitive::Flush
        )
    }

    /// Source: `createRoot(init)` is `createOwner()` then `runWithOwner(owner,
    /// init)` (`@solidjs/signals@2.0.0-rc.3` `dist/dev.js:2275-2281` for
    /// `createOwner`, which marks every owner it makes `_root: true`; the rc.9
    /// review § 3.3 finds the `createRoot` slice byte-identical, and rc.9's
    /// own `solid-js` export only forwards to it). So `init` runs during the
    /// call with that root as the ambient `context` the write guards read.
    ///
    /// Probed on the published rc.0-rc.9 triples, dev client builds: a signal
    /// setter, and `refresh(memo)`, directly in a `createRoot` body throw
    /// `REACTIVE_WRITE_IN_OWNED_SCOPE` on every release, whether the root is at
    /// module scope, in a component body, in a memo compute or in an effect
    /// apply. Prod builds carry no guard and throw nothing.
    ///
    /// Only `createRoot`. `runWithOwner(owner, fn)` also runs `fn` inline under
    /// an owner, but a supplied owner can be a leaf (`getOwner()` in a tracked
    /// effect), where every write is legal; `render`, `hydrate` and
    /// `createRevealOrder` create a root too but were not probed for this.
    fn callback_runs_in_created_root(&self, primitive: Primitive, argument: usize) -> bool {
        primitive == Primitive::CreateRoot && argument == 0
    }

    /// Source: `solid-js`'s dev `createComponent` is `devComponent`, which runs
    /// the component as `createRoot(() => { …; return untrack(() =>
    /// Comp(props)) }, { transparent: true })` (`solid-js@2.0.0-rc.3`
    /// `dist/dev.js:35-53` and `:1116-1118`; rc.0 the same shape; rc.9 renames
    /// it `observedComponent`, `dist/solid.dev.js:35-58` and `:1148-1150`).
    /// `untrack` keeps the owner, so the owner a write directly in the body
    /// meets is that root (`_root: true`, probed on rc.3 and rc.9). The prod
    /// build calls `untrack(() => Comp(props))` with no root, and carries no
    /// write guard either.
    ///
    /// Probed on every published rc.0-rc.9 dev client build, a store setter
    /// directly in a component body (inline, through `untrack` or `flush(fn)`,
    /// through a helper, and in a nested component) answers exactly as one in
    /// a `createRoot` body: `createStore`'s throws on rc.0 and rc.9 and not on
    /// rc.1-rc.8, `createOptimisticStore`'s throws on rc.9 only. A signal,
    /// optimistic-signal or action call there throws on every release.
    fn component_body_runs_under_root(&self) -> bool {
        true
    }

    /// Answered from the resolved `@solidjs/signals` (`releases.rs`, N3 of the
    /// rc.9 review). rc.1-rc.8's `devGuardStoreSetterWrite` opens with
    /// `if (context && !context._root && …)`; rc.9 drops `!context._root`
    /// (`dist/dev-shared.js:5878`, citing #3500), so `false` there only. rc.0
    /// has no setter-entry guard, and its `createStore` and
    /// `createOptimisticStore` setters disagree under a root (probed), so it
    /// keeps `true`, which claims nothing.
    fn store_setter_guard_exempts_roots(&self) -> bool {
        self.store_setter_roots == releases::StoreSetterRootGuard::Exempt
    }

    /// Answered from the resolved `@solidjs/signals` (`releases.rs`, N5):
    /// `false` on rc.0 only, whose optimistic store writes take the engine's
    /// path and meet no owned-scope guard (probed: legal in a memo and an
    /// effect compute on rc.0, `REACTIVE_WRITE_IN_OWNED_SCOPE` on rc.1-rc.9).
    fn optimistic_store_setter_guarded(&self) -> bool {
        self.optimistic_store_setter == releases::OptimisticStoreSetterGuard::Guarded
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

    /// The owner is a synchronous dynamic scope: `@solidjs/signals` sets it
    /// around each computation and `runWithOwner` and restores it before
    /// returning, so no owner is current on an empty stack. Probed on
    /// `solid-js`/`@solidjs/signals` 2.0.0-rc.3 and rc.9, dev and prod builds,
    /// in Chromium 153, for every one of the fourteen reviewed fresh-stack
    /// schedulers (`queueMicrotask`, `setTimeout`, `setInterval`,
    /// `requestAnimationFrame`, `requestIdleCallback`, `Promise.then`/`catch`/
    /// `finally`, `scheduler.postTask`, and the `IntersectionObserver`,
    /// `ResizeObserver`, `MutationObserver`, `PerformanceObserver` and
    /// `ReportingObserver` callbacks), each scheduled inside a `createRoot`,
    /// inside a `createMemo` compute and at module scope: `getOwner()` is
    /// `null` in the callback; an `onCleanup` there raises `NO_OWNER_CLEANUP`
    /// (dev) and never runs on the root's disposal; a `createEffect` there
    /// raises `NO_OWNER_EFFECT` (dev); an `onSettled` returning a cleanup
    /// raises `SETTLED_CLEANUP_UNOWNED` (dev) and the cleanup never runs.
    /// Control: a `createRoot` created in the callback owns its `onCleanup`,
    /// which runs on that root's disposal.
    fn fresh_stack_callback_owner(&self) -> Option<CallbackOwner> {
        Some(CallbackOwner::None)
    }

    /// `createStore` returns `Readonly<T>` over the root record
    /// (`@solidjs/signals@2.0.0-rc.0` and `rc.3`), so a write to one of its own
    /// properties is TS2540 and belongs to TypeScript. Nested records and props
    /// objects are not readonly and stay this checker's.
    ///
    /// From `@solidjs/signals@2.0.0-rc.7` it declares `Store<T> = T`: the
    /// write type-checks and the runtime still drops it, so a vocabulary built
    /// from such a signals answers `false` and SC2003 reports it
    /// (`releases.rs`).
    fn store_root_properties_are_readonly(&self) -> bool {
        self.store_root == releases::StoreRootTyping::Readonly
    }

    fn release_owners(&self) -> &'static [crate::ReleaseOwner] {
        releases::OWNERS
    }

    fn audited_installation(&self) -> &'static [(&'static str, &'static str)] {
        releases::AUDITED_INSTALLATION
    }

    fn review_installation(
        &self,
        installed: &[crate::InstalledRelease<'_>],
    ) -> crate::InstallationReview {
        releases::review(installed)
    }

    fn variant_key(&self) -> Option<&'static str> {
        self.key()
    }

    fn variants(&self) -> &'static [&'static dyn Dialect] {
        &releases::OTHER_VARIANTS
    }

    fn defaulted_vocabulary(&self) -> Option<&'static dyn Dialect> {
        Some(Solid2::DEFAULTED.interned())
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
            // `@solidjs/signals@2.0.0-rc.9` `until` (`dist/dev.js:2717-2785`,
            // `dist/prod/signals.js:530`): `new Promise(… createRoot(dispose =>
            // { …; effect(fn, …) }))`, the predicate is the compute of an
            // effect created under that fresh root -- except when
            // `options.signal` is already aborted, where it rejects before
            // `createRoot` and never runs `fn` at all (`:2734`). Either way no owner of
            // the caller's is used.
            Primitive::Until => &[(0, CallbackOwner::Creates)],
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
            // `createEffect` queues its apply (`effect()` enqueues every
            // `EFFECT_USER` run), so the apply always runs from the flush,
            // where no owner is current.
            Primitive::CreateEffect => &[(0, CallbackOwner::Creates), (1, CallbackOwner::None)],
            // `createRenderEffect` does not. `effect()` ends
            // `recompute(node, true); !options?.defer && (… || options?.schedule
            // ? node._queue.enqueue(…) : runEffect(node))`
            // (`@solidjs/signals@2.0.0-rc.3` `dist/dev.js:5285-5289`, the same
            // in `dist/prod/core/effect.js:16-17`; rc.9 `dist/dev.js:1612-1620`),
            // and `runEffect` sets neither `context` nor `tracking`, so the
            // first apply runs before the call returns under the caller's
            // owner. Probed on both prereleases, dev and prod: `getOwner()` in
            // the first apply is the enclosing root, a render effect created
            // there raises no `NO_OWNER_EFFECT`, and a setter there throws
            // `REACTIVE_WRITE_IN_OWNED_SCOPE` (dev builds) under a root or a
            // component.
            // Every later run comes from the flush, where `getOwner()` is
            // `null` and a render effect created there raises
            // `NO_OWNER_EFFECT`.
            Primitive::CreateRenderEffect => &[
                (0, CallbackOwner::Creates),
                (1, CallbackOwner::InheritsFirstRun),
            ],
            // `createReaction` is deliberately absent from this arm: 1.x
            // runs its invalidation callback as a leaf owner, but the
            // RC.0 runtime allocates the reaction a computation like
            // `createEffect` does, so 2.0 does not end the ownership chain
            // there. The `dialect-solid-2` fixture
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
            // The static form builds no memo and no wrapper computation: the
            // source is `untrack(source)` at the `dynamic` call itself
            // (`@solidjs/web@2.0.0-rc.9` `dist/web.dev.js:2199`,
            // `dist/server.js:3730`), under whatever owner the caller has.
            Primitive::DynamicStatic => &[(0, CallbackOwner::Inherits)],
            // `DynamicUnknownForm` is absent: its two runtimes disagree
            // (Creates versus Inherits), and absent is "unmodelled".
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

    /// Source: the published declarations, read from the exact package
    /// artifacts of the audited prerelease. In
    /// `solid-js@2.0.0-rc.3/types/server/signals.d.ts`:
    /// `createSignal<T>(value: Exclude<T, Function>, options?): Signal<T>`,
    /// `createMemo<T>(compute, options?): SourceAccessor<T>`, and
    /// `type SourceAccessor<T> = Refreshable<SignalAccessor<T>>`. `Signal` is
    /// re-exported from `@solidjs/signals@2.0.0-rc.3`, whose
    /// `dist/types/signals.d.ts` declares
    /// `type Signal<T> = [get: SourceAccessor<T>, set: Setter<T>]`. rc.3 is
    /// the prerelease this repository's fixtures and ecosystem corpus pin; the
    /// same three declarations were checked in rc.0 and rc.5 and are
    /// unchanged. `SourceAccessor` is one of the names
    /// [`Dialect::type_role`] already classifies as an accessor, so these rows
    /// restate an audited 2.0 declaration.
    ///
    /// The bundled 2.0 contract is *not* the authority for this question and
    /// disagrees in a way a row must not follow: `solidjs-signals.json`'s
    /// `createMemo` summary carries `output: "plain"` and its `createSignal`
    /// has no summary at all, because the generated single-value `returns`
    /// column cannot express either shape. That is the generator's silence,
    /// not the 2.0 vocabulary's negative claim.
    ///
    /// `createOptimistic` also returns a `Signal<T>` and `createProjection` a
    /// store; both stay absent until a proof needs them and their own review
    /// lands.
    fn reactive_result_slot(&self, primitive: Primitive, slot: ResultSlot) -> Option<ReactiveRole> {
        match (primitive, slot) {
            (Primitive::CreateSignal, ResultSlot::TupleItem(0)) => Some(ReactiveRole::Accessor),
            (Primitive::CreateSignal, ResultSlot::TupleItem(1)) => Some(ReactiveRole::Setter),
            (Primitive::CreateMemo, ResultSlot::Whole) => Some(ReactiveRole::Accessor),
            _ => None,
        }
    }
    /// ADR 0146. Source: `@solidjs/signals@2.0.0-rc.9`, the audited release
    /// (`dist/prod/signals.js` `createSignal`, `dist/prod/core/core.js`
    /// `signal` and `read`). `createSignal(e, t)` takes the memo path only when
    /// `typeof e === "function"`; otherwise it builds a plain `signal` node,
    /// whose only callbacks are `t?.equals` (called by the setter) and
    /// `t?.unobserved` (called when the last observer unlinks), both read off
    /// the options object. Its accessor is `read.bind(null, node)`, and `read`
    /// of a node with no compute function (`ce`), no firewall owner (`Te`) and
    /// no pending status serves the committed or staged value, linking the
    /// current observer when tracking: it runs no code. With every argument a
    /// primitive by grammar there is neither a function first argument nor an
    /// options object, so no path through it runs any. `createMemo`'s accessor
    /// recomputes its caller's function and is not stated.
    fn inert_accessor_read(&self, primitive: Primitive, slot: ResultSlot) -> bool {
        matches!(
            (primitive, slot),
            (Primitive::CreateSignal, ResultSlot::TupleItem(0))
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
    /// The effect apply at argument 1 is `Deferred` for both effect
    /// constructors, and for `createRenderEffect` that is an attribution
    /// answer, not a schedule. Its first apply runs *during* the call
    /// (see [`Solid2::callback_owners`]), with the caller's `tracking` still
    /// set, so a read there does subscribe an enclosing tracked computation
    /// (probed: a memo whose compute creates the render effect re-runs when a
    /// signal read only in that apply changes). What `Deferred` keeps true is
    /// the part the strict-read rule mirrors: `runEffect` opens the
    /// `"an effect callback"` strict-read window on every run, the first
    /// included, and a read in any later run subscribes nothing. The first
    /// run's owner, and with it the fact that its reads are the caller's, is
    /// stated by `callback_owners` (`CallbackOwner::InheritsFirstRun`, which
    /// the engine reads to keep those reads in the caller's summary), and its
    /// schedule by [`Solid2::contract_callback_execution_at`]; neither is read
    /// off this word.
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
            // The same attribution answer as `resolve`, on the same reading:
            // the predicate subscribes the effect `until` creates, never the
            // caller (who has no observer, or the call throws).
            | Primitive::Until
            | Primitive::Lazy
            | Primitive::Action
            | Primitive::ClientOnly => &[(0, Execution::Deferred)],
            Primitive::CreateRoot
            | Primitive::CreateRevealOrder
            | Primitive::Flush
            | Primitive::Untrack
            // `staticDynamic(untrack(source))`: invoked once, before
            // `dynamic` returns, and never again (`@solidjs/web@2.0.0-rc.9`
            // `dist/web.dev.js:2199`, `dist/web.js:2034`).
            | Primitive::DynamicStatic
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
    /// rc.0 install resolves — line numbers
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
    /// 2.0's package-contract words, moved off `interproc.rs`'s hardcoded
    /// table by ADR 0111.
    ///
    /// `createEffect` defers to the attribution answer: argument 0 is the
    /// tracked compute and argument 1 the effect function, which `effect()`
    /// always enqueues (`options.user` selects `EFFECT_USER`), so `deferred`
    /// is a true promise about it. Everything else is stated directly,
    /// because the contract word is not derivable from the attribution one --
    /// `onCleanup` carries no `callback_executions` row at all here and still
    /// promises `deferred` to a consumer.
    ///
    /// `createRenderEffect`'s apply has **no** word, and that is the answer
    /// the bytes support rather than a gap. `deferred` promises the callback
    /// has not run when the export returns, and the plain two-argument call
    /// runs it before returning (`@solidjs/signals@2.0.0-rc.3`
    /// `dist/dev.js:5285-5289`; probed on rc.3 and rc.9, dev and prod, inside
    /// no owner, a root, a memo compute, a render-effect compute and a
    /// component body under `render`: `compute,apply,returned` every time).
    /// `inline` would be false too: the same call leaves the first apply for
    /// later when `options.defer` (skipped) or `options.schedule` (queued)
    /// is set, when the compute returns a promise or reads a source that is
    /// still pending (probed: the apply runs after the source settles), and
    /// on rc.9 when the first pass was staged into a live transaction
    /// (`dist/dev.js:1617`); and every later run comes from the flush. A
    /// contract leaf left open is the fail-closed reading of both.
    ///
    /// Three arms of the old shared table are absent rather than ported:
    /// `createResource`, `on` and `mergeProps` are Solid 1.x names this
    /// dialect does not carry.
    fn contract_callback_execution_at(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> Option<Execution> {
        match (primitive, argument) {
            (Primitive::CreateEffect, _) | (Primitive::CreateRenderEffect, 0) => {
                self.callback_execution_at(primitive, argument, argument_count)
            }
            (
                Primitive::CreateMemo
                | Primitive::CreateTrackedEffect
                | Primitive::CreateSignal
                | Primitive::CreateStore
                | Primitive::CreateProjection
                | Primitive::CreateOptimistic
                | Primitive::CreateOptimisticStore
                | Primitive::Dynamic,
                0,
            ) => Some(Execution::Tracked),
            (
                Primitive::OnSettled
                | Primitive::Action
                | Primitive::CreateReaction
                | Primitive::OnCleanup,
                0,
            ) => Some(Execution::Deferred),
            // The static `dynamic` form is `untrack(source)` at the call
            // (`@solidjs/web@2.0.0-rc.9` `dist/web.dev.js:2199`,
            // `dist/server.js:3730`): the source has run and returned before
            // `dynamic` does, so it is `inline`, with `untrack`'s clearing
            // travelling separately exactly as it does for `untrack`.
            (
                Primitive::CreateRoot
                | Primitive::Untrack
                | Primitive::Flush
                | Primitive::DynamicStatic,
                0,
            )
            | (Primitive::RunWithOwner, 1) => Some(Execution::Inline),
            // Deliberately unanswered, so a forwarded parameter opens the
            // unknown-callback sentinel:
            //
            // - `DynamicUnknownForm`: `tracked` or `inline` depending on a
            //   value the syntax does not prove.
            // - `until`: its predicate runs during the call (the effect's
            //   first compute), again after it whenever a dependency changes
            //   until one run is truthy, and not at all when `options.signal`
            //   is already aborted (`@solidjs/signals@2.0.0-rc.9`
            //   `dist/dev.js:2734`). No one word is that; `resolve`,
            //   the same shape, is unanswered for the same reason.
            _ => None,
        }
    }

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
    /// `createEffect(compute, apply)` and `createRenderEffect(compute, apply)`
    /// are the two with an apply slot; `createTrackedEffect(compute, options?)`
    /// has none. Matches the `(1, Execution::Deferred)` row in
    /// `callback_executions`, which is the same fact seen from the attribution
    /// side.
    fn apply_callback_argument(&self, primitive: Primitive) -> Option<usize> {
        matches!(
            primitive,
            Primitive::CreateEffect | Primitive::CreateRenderEffect
        )
        .then_some(1)
    }

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
    /// 2.0 renamed `mergeProps` to `merge`. `omit` and `pick`, the
    /// `splitProps` replacements, are deliberately absent for the same reason
    /// 1.x leaves `splitProps` out: their result is a tuple, not the merged
    /// object.
    fn merges_props_reactivity(&self, primitive: Primitive) -> bool {
        primitive == Primitive::Merge
    }

    /// 2.0 replaced `splitProps` with `omit`; `store/utils.d.ts` declares
    /// `omit(props: T, ...keys: K)`, the same props-plus-key-lists shape.
    /// rc.9's predicate overload is the one argument this does not cover; see
    /// [`Solid2::callback_runs_on_result_access`].
    fn splits_props(&self, primitive: Primitive) -> bool {
        primitive == Primitive::Omit
    }

    /// `@solidjs/signals@2.0.0-rc.9` adds
    /// `omit(props: T, hidden: (key: keyof T & (string | symbol)) => boolean):
    /// Partial<T>` (`dist/types/store/utils.d.ts:251`), and the runtime picks
    /// that form by
    /// `keys.length === 1 && typeof keys[0] === "function"` (`dist/dev.js:4380`,
    /// the same test at `dist/prod/store/utils.js:982` and
    /// `dist/observe/store/utils.js:984`; `solid-js` re-exports this `omit`,
    /// server build included, `dist/server.js:2`). So only a call with exactly
    /// two arguments can carry a predicate, and it is argument 1.
    ///
    /// When it runs, read from the same bytes:
    ///
    /// - **Wherever `Proxy` exists** (`SUPPORTS_PROXY = typeof Proxy ===
    ///   "function"`, `dist/dev-shared.js:273`), `omit` only stores it:
    ///   `new Proxy(new OmitView(source, kind, hidden), omitTraps)`
    ///   (`dist/dev.js:4406`). `isHidden(view, key)` then calls it on every
    ///   `get`, `has` and key enumeration of the returned view
    ///   (`omitTraps`, `:4178-4241`; `isHidden`, `:3495-3498`), and on every
    ///   walk of the view by `merge` and the spread helpers (`:3557-3703`,
    ///   `:3826-3870`, `:3984`, `:4071`, `:4113-4128`). Nothing around those
    ///   calls touches the listener, so the predicate's reads subscribe
    ///   whatever computation is reading the view, and its writes run under
    ///   that reader's owner.
    /// - **Without `Proxy`**, it is called once per own property name during
    ///   the `omit` call itself, in the caller's scope (`:4408-4427`).
    ///
    /// Neither is a schedule the `Execution` vocabulary can state, and the
    /// first is the one every supported runtime takes.
    ///
    /// Answered from the resolved `@solidjs/signals`
    /// (`Solid2::omit_predicate_form`). rc.3's `omit` has no predicate form:
    /// `typeof keys[0]` is never read and every build only tests membership
    /// (`keys.includes(key)`, rc.3 `dist/dev.js:9334-9369`), and its
    /// declaration rejects a function there (TS2345). So the audited
    /// vocabulary answers `false` -- for typed code that is TypeScript's
    /// boundary, and for an untyped artifact it is what rc.3's bytes do -- and
    /// only a vocabulary built from signals rc.9 answers the predicate slot.
    fn callback_runs_on_result_access(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> bool {
        self.omit_predicate_form
            && primitive == Primitive::Omit
            && argument == 1
            && argument_count == 2
    }

    /// `action(genFn)`'s generator, argument 0. Every release the reviews read
    /// drives it the same way: `const it = genFn(...args)` inside the returned
    /// wrapper's promise executor, then `step()`, which runs `it.next(v)` (or
    /// `it.throw(v)` for a rejected yielded thenable) and settles or schedules
    /// the next step from the result (`@solidjs/signals@2.0.0-rc.9`
    /// `dist/dev.js:1961-2066`). From rc.8 `step` brackets that call with
    /// `enterActionStep()`/`exitActionStep()`; the stepping itself is older,
    /// and answering it on every release is what lets the throw below be the
    /// one release-keyed fact.
    fn callback_runs_as_action_steps(&self, primitive: Primitive, argument: usize) -> bool {
        primitive == Primitive::Action && argument == 0
    }

    /// `createComponent(Comp, props)`, argument 0, on every release the
    /// reviews read. The export is `solid-js`'s; `@solidjs/web` re-exports it
    /// unchanged in every build (`export { …, createComponent, … } from
    /// 'solid-js'`, rc.3 and rc.9 `dist/web.js:2`, `dist/server.js:2`, and the
    /// dev and observe builds). Each `solid-js` build calls `Comp` exactly once,
    /// synchronously, before it returns, and retains nothing it later invokes:
    ///
    /// | build | rc.0-rc.9 | owner `Comp` runs under |
    /// | --- | --- | --- |
    /// | client prod (`solid.js`; rc.9 `:1122-1124`, rc.3 `:1095-1097`) | `untrack(() => Comp(props \|\| {}))` | the caller's |
    /// | client dev (`dev.js`/`solid.dev.js`; rc.3 `devComponent` `:35-53`, rc.9 `observedComponent` `:35-57`) | `createRoot(() => untrack(() => Comp(props)), { transparent: true })` | a transparent child root of the caller's |
    /// | client observe (rc.8, rc.9 `solid.observe.js:36-46`) | as dev, without the dev checks | as dev |
    /// | server prod (`server.js`; rc.9 `:1644-1646`, rc.3 `:1466-1468`), and rc.7/rc.8's server dev and observe | `Comp(props \|\| {})` | the caller's |
    /// | server dev/observe (rc.9 `server.dev.js:1748-1755`, `server.observe.js:1721-1725`) | `runWithOwner(createComponentOwner(…), () => Comp(props \|\| {}))`, or `Comp(props \|\| {})` with no owner | a child of the caller's |
    ///
    /// `createRoot` runs its function in place (`runWithOwner(owner, …)` on
    /// the caller's stack), so no build defers the call. Every one of the ten
    /// releases' builds was read for the three shapes above.
    /// The dev builds also store `Comp` on the owner (`owner._component = { fn:
    /// Comp, … }`) and tag it with `$DEVCOMP`; no build of `solid-js`,
    /// `@solidjs/signals` or `@solidjs/web` at rc.3 or rc.9 reads
    /// `_component` back, so neither is a second entry. Probed on the
    /// published triples (`createComponent` inside a memo, then a write to a
    /// signal `Comp` read): every one of rc.3's four builds and rc.9's six ran
    /// `Comp` once, between the statements before and after the call, and
    /// never again.
    fn renders_component_argument(&self, name: &str) -> Option<usize> {
        (name == "createComponent").then_some(0)
    }

    /// ADR 0153, read on every bundle of the audited `solid-js` archives
    /// (rc.3 and rc.9: `dist/solid.js`, `dist/dev.js`, `dist/server.js`, and
    /// rc.9's observe variants) and on `@solidjs/signals` rc.3, rc.6 and rc.9
    /// (`dist/prod/core/context.js`):
    ///
    /// ```js
    /// function createContext(defaultValue, options) {
    ///   const id = Symbol(options && options.name || "");
    ///   function provider(props) {
    ///     return createRoot(() => {
    ///       setContext(provider, props.value);
    ///       return children(() => props.children);
    ///     });
    ///   }
    ///   provider.id = id;
    ///   provider.defaultValue = defaultValue;
    ///   return provider;
    /// }
    /// function useContext(context) { return getContext(context); }
    /// function createComponent(Comp, props) { return untrack(() => Comp(props || {})); }
    /// ```
    ///
    /// `getContext(e, t = getOwner())` throws `NoOwnerError` without an owner,
    /// answers the owner's map at `e.id` (rc.9 `t.ze[e.id]`, rc.3 `hasContext`
    /// then `t.we[e.id]`) or else `e.defaultValue`, and throws
    /// `ContextNotFoundError` when that is `undefined`; the server bundle's
    /// `useContext` rethrows the same error. `setContext(e, t, r = getOwner())`
    /// stores `t`, or `e.defaultValue` when `t` is `undefined`, under `e.id` in a
    /// copy of the owner's map. The server `createComponent` is
    /// `Comp(props || {})` without `untrack`. So a context made with no
    /// arguments reads only values stored under its fresh symbol, and storing
    /// one takes the context object itself.
    fn context_role(&self, name: &str) -> Option<ContextRole> {
        match name {
            "createContext" => Some(ContextRole::Create),
            "useContext" => Some(ContextRole::Read),
            "createComponent" => Some(ContextRole::Render),
            _ => None,
        }
    }

    /// `Dynamic`'s `component`, on every release the reviews read. `Dynamic`
    /// is `@solidjs/web`'s, the same function in every build of rc.3 and rc.9
    /// (rc.3 `web.js:1865`, `dev.js:1935`, `server.js:3208`; rc.9 `web.js:2099`,
    /// `web.dev.js:2271`, `web.observe.js:2121`, `server.js:3764`,
    /// `server.dev.js:4013`, `server.observe.js:3852`):
    ///
    /// ```js
    /// function Dynamic(props) {
    ///   const Comp = dynamic(() => props.component);
    ///   return createComponent(Comp, omit(props, "component"));
    /// }
    /// ```
    ///
    /// `dynamic(source)` keeps the value in a memo it creates (`cached`) and
    /// returns a component that, rendered, creates one more memo reading it:
    /// a function is called there as `component(props)` (or its
    /// `Symbol.for("solid.component-binding")` target, when it carries one),
    /// a string builds an element, and nothing else is done with it. The value
    /// reaches no other invocation: `bindingOf` and the thenable test read a
    /// property of it, and the dev and observe builds' `Object.assign(component,
    /// { [$DEVCOMP]: true })` writes one. The `props` it receives are
    /// `omit(props, "component")`. `createComponent` runs `Dynamic`, and then
    /// `Comp`, in place (ADR 0136), so the component is invoked only inside
    /// computations the render creates.
    ///
    /// Probed on the published triples: `createComponent(Dynamic, { get
    /// component() { return Selected(); } })` with `Selected = createMemo(() =>
    /// flag() ? A : B)`, inside a root. In all six rc.9 builds and rc.3's
    /// four, `A` ran once, between the statements before and after the call,
    /// with no `component` in its props, under an owner below the render's
    /// (one level in prod, three in dev and observe; rc.3's client prod owners
    /// do not expose the link). The client builds ran `B` once when `flag` was
    /// written and flushed, under the same owner depth; the server builds,
    /// which do not re-run, never did. A data property (`{ component: A }`)
    /// behaved the same.
    fn component_prop_renderers(&self) -> &'static [(&'static str, &'static str)] {
        &[("Dynamic", "component")]
    }

    /// `createMemo(compute)`, one argument, `compute` taking no parameters. The
    /// value `compute` returns is the node's value, and the node is reachable
    /// from program code only through the accessor `createMemo` returns
    /// (`accessor(computed(compute, undefined))` in `@solidjs/signals`, rc.9
    /// `prod/signals.js:77`; the accessor is `read.bind(null, node)`, and its
    /// `$REFRESH` slot re-runs `compute`). With no options there is no
    /// `loadingValue`, `equals` or `ssrSource`; a parameterless compute cannot
    /// read its previous value. `solid-js`'s client `createMemo` is the
    /// hydration wrapper (rc.9 `solid.js:596-601`, `770-772`): while hydrating
    /// it may yield a serialized value instead of calling `compute`, which is
    /// another value, never a second path for this one. The server
    /// `createMemo` runs `compute` in place.
    fn accessor_yields_only_its_compute(&self, primitive: Primitive) -> bool {
        primitive == Primitive::CreateMemo
    }

    /// `flush` inside an action step: the `FLUSH_IN_ACTION` dev throw, on the
    /// releases that have it (`Solid2::flush_in_action`).
    fn throws_inside_action_step(&self, primitive: Primitive) -> bool {
        self.flush_in_action && primitive == Primitive::Flush
    }

    /// `createSignal` and `createOptimistic` both return
    /// `Signal<T> = [get: SourceAccessor<T>, set: Setter<T>]`; `createStore`
    /// and `createOptimisticStore` both return
    /// `[get: Store<T>, set: StoreSetter<T>]`. `createProjection` returns the
    /// store itself, so it is deliberately absent, and `createResource` is not
    /// 2.0 vocabulary at all.
    fn returns_reactive_tuple(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateSignal
                | Primitive::CreateStore
                | Primitive::CreateOptimistic
                | Primitive::CreateOptimisticStore
        )
    }

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
        if name == "until" && !self.until {
            return Vec::new();
        }
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
/// The invariant, which `solid_1x.rs` enforced too before the 1.x dialect was
/// retired: every modelled primitive
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
    "until",
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

    /// The package-contract words this dialect states, pinned exactly.
    ///
    /// These lived in `solid-reactive-ir`'s `interproc.rs` as a hardcoded
    /// `match` until ADR 0111, where shared code decided per-version
    /// behaviour and a new dialect could not state a different answer. Moving
    /// them here changed no contract in the corpus; this test is what keeps
    /// the move honest.
    ///
    /// The contract word is **not** derivable from
    /// [`Dialect::callback_execution_at`], which is why the two are asserted
    /// apart: `onCleanup` carries no `callback_executions` row at all and
    /// still promises `deferred` to a consumer.
    #[test]
    fn the_contract_words_are_stated_by_this_dialect_not_by_shared_code() {
        let two = &Solid2 as &dyn Dialect;
        let word = |primitive, argument, count| {
            two.contract_callback_execution_at(primitive, argument, count)
        };

        for primitive in [
            Primitive::CreateMemo,
            Primitive::CreateTrackedEffect,
            Primitive::CreateSignal,
            Primitive::CreateStore,
            Primitive::CreateProjection,
            Primitive::CreateOptimistic,
            Primitive::CreateOptimisticStore,
            Primitive::Dynamic,
        ] {
            assert_eq!(
                word(primitive, 0, 1),
                Some(Execution::Tracked),
                "{primitive:?}"
            );
        }
        for primitive in [
            Primitive::OnSettled,
            Primitive::Action,
            Primitive::CreateReaction,
            Primitive::OnCleanup,
        ] {
            assert_eq!(
                word(primitive, 0, 1),
                Some(Execution::Deferred),
                "{primitive:?}"
            );
        }
        for primitive in [Primitive::CreateRoot, Primitive::Untrack, Primitive::Flush] {
            assert_eq!(
                word(primitive, 0, 1),
                Some(Execution::Inline),
                "{primitive:?}"
            );
        }
        // The clearing wrapper whose callback slot is not index 0.
        assert_eq!(word(Primitive::RunWithOwner, 1, 2), Some(Execution::Inline));
        assert_eq!(word(Primitive::RunWithOwner, 0, 2), None);

        // `createEffect` defers to the attribution answer for both arguments:
        // a tracked compute and a queued effect function.
        assert_eq!(
            word(Primitive::CreateEffect, 0, 2),
            Some(Execution::Tracked)
        );
        assert_eq!(
            word(Primitive::CreateEffect, 1, 2),
            Some(Execution::Deferred)
        );
        assert_eq!(
            word(Primitive::CreateRenderEffect, 0, 2),
            Some(Execution::Tracked)
        );
        // `createRenderEffect`'s apply runs before the call returns on the
        // plain path and after it under `defer`/`schedule`, an async or
        // pending compute, or (rc.9) a staged transaction, so neither `inline`
        // nor `deferred` is a true promise at any arity. The attribution word
        // stays `Deferred`; only the contract refuses to restate it.
        for count in 2..=3 {
            assert_eq!(word(Primitive::CreateRenderEffect, 1, count), None);
        }
        assert_eq!(
            two.callback_execution_at(Primitive::CreateRenderEffect, 1, 2),
            Some(Execution::Deferred)
        );

        // `onCleanup` is the case that proves the word is stated, not derived.
        assert!(two.callback_executions(Primitive::OnCleanup).is_empty());
        assert_eq!(word(Primitive::OnCleanup, 0, 1), Some(Execution::Deferred));

        // Silence is an answer contract emission must keep as "unknown".
        assert_eq!(word(Primitive::Children, 0, 1), None);
    }

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
                "until",
                "untrack",
                "useContext",
                "useHead",
            ]
        );
    }

    /// The `dynamic` call forms, pinned against `@solidjs/web@2.0.0-rc.9`'s
    /// `if (options?.static)` (`dist/web.dev.js:2199`): only a proven literal
    /// picks a form, a proven falsy or absent option keeps the default, and an
    /// unproven one reaches the form that states nothing.
    #[test]
    fn dynamic_call_forms_follow_the_static_option_literal() {
        use crate::OptionLiteral;
        let form = |vocabulary: Solid2, literal: OptionLiteral| {
            vocabulary.call_form(Primitive::Dynamic, &|argument, key| {
                assert_eq!((argument, key), (1, "static"));
                literal
            })
        };
        let rc9 = Solid2::RC9;
        assert_eq!(form(rc9, OptionLiteral::True), Primitive::DynamicStatic);
        assert_eq!(form(rc9, OptionLiteral::False), Primitive::Dynamic);
        assert_eq!(form(rc9, OptionLiteral::Absent), Primitive::Dynamic);
        assert_eq!(
            form(rc9, OptionLiteral::Unknown),
            Primitive::DynamicUnknownForm
        );
        // rc.0-rc.8's web never reads the option: every call is the default,
        // which is what those runtimes build.
        for literal in [
            OptionLiteral::True,
            OptionLiteral::False,
            OptionLiteral::Absent,
            OptionLiteral::Unknown,
        ] {
            assert_eq!(form(Solid2, literal), Primitive::Dynamic);
        }
        // A web release nobody read: an option-bearing call states nothing.
        let unread = Solid2 {
            dynamic_options: releases::DynamicOptions::Unread,
            ..Solid2
        };
        assert_eq!(
            form(unread, OptionLiteral::True),
            Primitive::DynamicUnknownForm
        );
        assert_eq!(
            form(unread, OptionLiteral::Unknown),
            Primitive::DynamicUnknownForm
        );
        assert_eq!(form(unread, OptionLiteral::False), Primitive::Dynamic);
        assert_eq!(form(unread, OptionLiteral::Absent), Primitive::Dynamic);
        // Nothing else has a form, and nothing else is asked about options.
        assert_eq!(
            Solid2.call_form(Primitive::Untrack, &|_, _| panic!("not asked")),
            Primitive::Untrack
        );
        // A form spells as the export it is a call of, and is never a name.
        for primitive in [Primitive::DynamicStatic, Primitive::DynamicUnknownForm] {
            assert_eq!(Solid2.name_of(primitive), Some("dynamic"));
        }
        assert_eq!(Solid2.primitive("dynamic"), Some(Primitive::Dynamic));
    }

    /// The static form answers exactly as `untrack` does, because it *is*
    /// `untrack(source)` at the call; the default keeps its rc.3 rows; the
    /// unknown form states nothing a consumer could rely on.
    #[test]
    fn dynamic_forms_answer_their_own_runtime() {
        let two: &dyn Dialect = &Solid2::RC9;
        for primitive in [Primitive::DynamicStatic, Primitive::Untrack] {
            assert_eq!(
                two.callback_execution_at(primitive, 0, 2),
                Some(Execution::Inline)
            );
            assert!(two.runs_callback_deferred(primitive));
            assert!(two.runs_callback_synchronously(primitive));
            assert!(two.callback_preserves_owner_write_context(primitive));
            assert_eq!(
                two.callback_owner_at(primitive, 0, 2),
                Some(CallbackOwner::Inherits)
            );
            assert_eq!(
                two.contract_callback_execution_at(primitive, 0, 2),
                Some(Execution::Inline)
            );
        }
        assert_eq!(
            two.callback_execution_at(Primitive::Dynamic, 0, 1),
            Some(Execution::Tracked)
        );
        assert_eq!(
            two.callback_owner_at(Primitive::Dynamic, 0, 1),
            Some(CallbackOwner::Creates)
        );
        assert_eq!(
            two.contract_callback_execution_at(Primitive::Dynamic, 0, 1),
            Some(Execution::Tracked)
        );
        let unknown = Primitive::DynamicUnknownForm;
        assert_eq!(two.callback_execution_at(unknown, 0, 2), None);
        assert_eq!(two.callback_owner_at(unknown, 0, 2), None);
        assert_eq!(two.contract_callback_execution_at(unknown, 0, 2), None);
        assert!(!two.runs_callback_deferred(unknown));
    }

    /// rc.9's `until` is `resolve`'s shape (`dist/dev.js:2717-2785`), so it
    /// answers `resolve`'s rows, contract silence included -- on an
    /// installation that exports it. On the audited rc.3 triple the name is
    /// not an export at all (TS2305), so the vocabulary does not know it.
    #[test]
    fn until_answers_as_resolve_does() {
        let audited: &dyn Dialect = &Solid2;
        assert_eq!(audited.primitive("until"), None);
        assert_eq!(audited.name_of(Primitive::Until), None);
        assert!(
            audited
                .export_modules("until", crate::ExportPosition::Value)
                .is_empty()
        );
        let two: &dyn Dialect = &Solid2::RC9;
        assert_eq!(two.primitive("until"), Some(Primitive::Until));
        assert_eq!(two.name_of(Primitive::Until), Some("until"));
        for primitive in [Primitive::Until, Primitive::Resolve] {
            assert_eq!(two.callback_positions(primitive), &[0]);
            assert_eq!(
                two.callback_execution_at(primitive, 0, 2),
                Some(Execution::Deferred)
            );
            assert!(two.runs_callback_deferred(primitive));
            assert_eq!(
                two.callback_owner_at(primitive, 0, 2),
                Some(CallbackOwner::Creates)
            );
            assert_eq!(two.contract_callback_execution_at(primitive, 0, 2), None);
        }
        assert_eq!(
            two.export_modules("until", crate::ExportPosition::Value),
            vec!["solid-js"]
        );
        assert!(
            two.namespace_import_primitives("solid-js")
                .contains(&"until")
        );
    }

    /// Only a two-argument `omit` can carry rc.9's predicate
    /// (`keys.length === 1 && typeof keys[0] === "function"`,
    /// `dist/dev.js:4380`), and only at argument 1. rc.3 has no predicate
    /// form at all.
    #[test]
    fn only_the_two_argument_omit_slot_runs_on_result_access() {
        let audited: &dyn Dialect = &Solid2;
        for argument_count in 0..4 {
            for argument in 0..argument_count {
                assert!(!audited.callback_runs_on_result_access(
                    Primitive::Omit,
                    argument,
                    argument_count
                ));
            }
        }
        assert!(!Solid2::RC3.callback_runs_on_result_access(Primitive::Omit, 1, 2));
        assert!(!Solid2::default().callback_runs_on_result_access(Primitive::Omit, 1, 2));
        let two: &dyn Dialect = &Solid2::RC9;
        assert!(two.callback_runs_on_result_access(Primitive::Omit, 1, 2));
        assert!(!two.callback_runs_on_result_access(Primitive::Omit, 0, 2));
        assert!(!two.callback_runs_on_result_access(Primitive::Omit, 1, 3));
        assert!(!two.callback_runs_on_result_access(Primitive::Omit, 2, 3));
        assert!(!two.callback_runs_on_result_access(Primitive::Merge, 1, 2));
        // The split row itself is unchanged: the exception is the slot's.
        assert!(two.splits_props(Primitive::Omit));
        assert_eq!(two.callback_execution_at(Primitive::Omit, 1, 2), None);
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

    /// The namespace invariant, which the retired `solid_1x.rs` also held: a
    /// namespace import must
    /// retain every modelled runtime obligation the module exports, so a
    /// primitive cannot be reachable as `import { x }` but invisible as
    /// `Solid.x`.
    #[test]
    fn every_modelled_export_resolves_through_its_namespace_module() {
        // The namespace lists are static and name every release's exports; a
        // name an installation lacks is then simply not a primitive there.
        for vocabulary in [Solid2, Solid2::RC9] {
            for module in vocabulary.modules() {
                let mut expected = TABLE
                    .iter()
                    .filter_map(|(name, _)| {
                        vocabulary
                            .export_modules(name, crate::ExportPosition::Value)
                            .contains(module)
                            .then_some(*name)
                    })
                    .collect::<Vec<_>>();
                expected.sort_unstable();
                let mut actual = vocabulary
                    .namespace_import_primitives(module)
                    .iter()
                    .copied()
                    .filter(|name| {
                        !RELEASE_GATED_NAMES.contains(name) || vocabulary.primitive(name).is_some()
                    })
                    .collect::<Vec<_>>();
                actual.sort_unstable();
                assert_eq!(
                    actual, expected,
                    "namespace imports from {module} must retain every modelled runtime obligation"
                );
            }
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

    /// The one audited archive `name@version`.
    fn archive(name: &str, version: &str) -> &'static AuditedArchive {
        let mut matching = AUDITED_ARCHIVES
            .iter()
            .filter(|archive| archive.name == name && archive.version == version);
        let found = matching
            .next()
            .unwrap_or_else(|| panic!("{name}@{version} is not an audited archive"));
        assert!(
            matching.next().is_none(),
            "{name}@{version} is listed twice"
        );
        found
    }

    /// Repository root, from this crate's manifest directory.
    fn repository_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .canonicalize()
            .unwrap()
    }

    /// Every audited 2.0 document, as `(repository-relative path, bytes)`.
    ///
    /// Enumerated from the directory rather than listed, so a document added
    /// to the bundle is picked up by the completeness test below instead of
    /// silently escaping it.
    fn audited_documents() -> Vec<(String, Vec<u8>)> {
        let root = repository_root();
        let directory = root.join("pkg/contracts/bundled/solid-v2");
        let mut documents = Vec::new();
        for entry in std::fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if !name.ends_with(".json") || name.contains("receipt") || name == "bundle-index.json" {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            // The crate's review copy must be the same bytes; the runtime copy
            // is what the analyzer loads, so a row cites that one and this
            // asserts the review location has not drifted from it.
            let mirror = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("contracts")
                .join("solid-v2")
                .join(&name);
            assert_eq!(
                std::fs::read(&mirror).unwrap(),
                bytes,
                "review copy of {name} differs from the runtime copy"
            );
            documents.push((format!("pkg/contracts/bundled/solid-v2/{name}"), bytes));
        }
        documents.sort_by(|left, right| left.0.cmp(&right.0));
        documents
    }

    /// Whether one normalized `call` object closes `domain` with an empty
    /// collection — the fact a negative row restates.
    fn closes_domain_empty(call: &serde_json::Value, domain: CallClaimDomain) -> bool {
        let name = domain.wire_name();
        let items = call[name].as_array();
        let closed = call["closed"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|entry| entry.as_str() == Some(name));
        items.is_some_and(|items| items.is_empty()) && closed
    }

    /// The rows this table deliberately does not carry even though some audited
    /// source supports deriving them. Each entry's reason is in
    /// [`NEGATIVE_ROWS`]' doc comment; this list exists so the completeness
    /// test below cannot be satisfied by an accidental omission.
    ///
    /// A withholding must be *derivable* from somewhere — otherwise it is a
    /// note about nothing — so every entry here is either closed by an audited
    /// contract document or listed in [`IMPLEMENTATION_AUDITED`].
    const WITHHELD: &[(&str, &str, &str, CallClaimDomain)] = &[
        (
            "@solidjs/signals",
            RC6,
            "createOptimisticStore",
            CallClaimDomain::Reads,
        ),
        (
            "@solidjs/signals",
            RC9,
            "createOptimisticStore",
            CallClaimDomain::Reads,
        ),
        ("@solidjs/web", RC3, "hydrate", CallClaimDomain::Creates),
        ("@solidjs/web", RC3, "render", CallClaimDomain::Reads),
        ("@solidjs/web", RC3, "hydrate", CallClaimDomain::Reads),
        ("solid-js", RC9, "Show", CallClaimDomain::Reads),
        ("solid-js", RC9, "Show", CallClaimDomain::Creates),
        ("solid-js", RC9, "Loading", CallClaimDomain::Creates),
        ("solid-js", RC9, "createSignal", CallClaimDomain::Creates),
        ("solid-js", RC9, "createMemo", CallClaimDomain::Creates),
        ("solid-js", RC9, "createSignal", CallClaimDomain::Reads),
        ("solid-js", RC9, "createMemo", CallClaimDomain::Reads),
        ("@solidjs/signals", RC9, "merge", CallClaimDomain::Creates),
        ("solid-js", RC9, "affects", CallClaimDomain::Reads),
        ("solid-js", RC9, "affects", CallClaimDomain::Creates),
        ("solid-js", RC9, "isPending", CallClaimDomain::Creates),
        ("solid-js", RC9, "latest", CallClaimDomain::Creates),
        ("solid-js", RC9, "refresh", CallClaimDomain::Reads),
        ("solid-js", RC9, "refresh", CallClaimDomain::Creates),
        ("@solidjs/web", RC9, "hydrate", CallClaimDomain::Reads),
        ("@solidjs/web", RC9, "render", CallClaimDomain::Reads),
        ("solid-js", RC3, "createEffect", CallClaimDomain::Creates),
        ("solid-js", RC3, "createEffect", CallClaimDomain::Reads),
        ("solid-js", RC3, "createSignal", CallClaimDomain::Creates),
        ("solid-js", RC3, "Show", CallClaimDomain::Reads),
        ("solid-js", RC3, "Show", CallClaimDomain::Creates),
        ("solid-js", RC3, "Loading", CallClaimDomain::Creates),
    ];

    /// The reading each [`RowScope::HostTarget`] row rests on: the audit, the
    /// section heading that walked the scoped condition, and that condition.
    /// One entry per scoped row, and the row's citations must name the same
    /// section, so a scoped row cannot borrow the section that withheld its
    /// flat twin.
    const HOST_TARGET_READINGS: &[(
        &str,
        &str,
        &str,
        CallClaimDomain,
        &str,
        &str,
        HostTargetCondition,
    )] = &[
        (
            "solid-js",
            RC3,
            "createSignal",
            CallClaimDomain::Creates,
            RC3_CORE_PRIMITIVES_AUDIT,
            "### 7.3 R6 — `solid-js`' own `createSignal`, browser conditions",
            HostTargetCondition::Browser,
        ),
        (
            "solid-js",
            RC9,
            "createSignal",
            CallClaimDomain::Creates,
            RC9_CORE_WEB_AUDIT,
            "### 13. `createSignal` — `creates`, `browser` host target — archive `solid-js@2.0.0-rc.9`",
            HostTargetCondition::Browser,
        ),
        (
            "solid-js",
            RC9,
            "createMemo",
            CallClaimDomain::Creates,
            RC9_MERGE_OMIT_MEMO_AUDIT,
            "## 4. `createMemo` — `creates`, `browser` host target — archive `solid-js@2.0.0-rc.9`",
            HostTargetCondition::Browser,
        ),
    ];

    /// What a hand implementation census concluded, per row.
    ///
    /// A second **source** for the derivation below, beside the audited JSON
    /// documents — not a second kind of row. `Closed` makes a row derivable
    /// exactly as a document's closure does; `Withheld` makes a withholding
    /// legitimate without making a row derivable, so the "neither shipped nor
    /// withheld" omission check keeps working for an export no document
    /// mentions.
    ///
    /// The JSON half of the derivation stays exactly as strict as it was: an
    /// entry here can only *add* to what is derivable, and adding one is a
    /// deliberate edit that names its audit document and section.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ImplementationVerdict {
        Closed,
        Withheld(&'static str),
    }

    const IMPLEMENTATION_AUDITED: &[(
        &str,
        &str,
        &str,
        CallClaimDomain,
        &str,
        &str,
        ImplementationVerdict,
    )] = &[
        // R1-R5 of RC3_CORE_PRIMITIVES_AUDIT § 0. Each was censused in all
        // three `@solidjs/signals` bundles, and — because the tier binds the
        // declaration's archive while a `solid-js` import under the `node`
        // condition runs `solid-js/dist/server.js`'s own bodies (§ 1.4) —
        // those server bodies were read too and reach the same verdict:
        // `getOwner` `dist/server.js:91-93`, `onCleanup` `:97-102`,
        // `createRoot` `:175-178`, `untrack` `:1392-1394` (file digest
        // `63269da7…`). `createSignal`'s server body is the exception, and it
        // is why the `solid-js` row below is withheld rather than granted.
        (
            "@solidjs/signals",
            RC3,
            "createRoot",
            CallClaimDomain::Creates,
            RC3_CORE_PRIMITIVES_AUDIT,
            "## 3. `createRoot` — archive `@solidjs/signals@2.0.0-rc.3`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC3,
            "createSignal",
            CallClaimDomain::Creates,
            RC3_CORE_PRIMITIVES_AUDIT,
            "## 7. `createSignal` — two archives, two rows",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC3,
            "getOwner",
            CallClaimDomain::Creates,
            RC3_CORE_PRIMITIVES_AUDIT,
            "## 4. `getOwner` — archive `@solidjs/signals@2.0.0-rc.3`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC3,
            "onCleanup",
            CallClaimDomain::Creates,
            RC3_CORE_PRIMITIVES_AUDIT,
            "## 5. `onCleanup` — archive `@solidjs/signals@2.0.0-rc.3`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC3,
            "untrack",
            CallClaimDomain::Creates,
            RC3_CORE_PRIMITIVES_AUDIT,
            "## 6. `untrack` — archive `@solidjs/signals@2.0.0-rc.3`",
            ImplementationVerdict::Closed,
        ),
        // RC3_OWNER_CONTEXT_AUDIT, 2026-09-23. `runWithOwner` is declared in
        // `@solidjs/signals` and runs `solid-js/dist/server.js:82-90` under the
        // `node` condition, which the audit reads too (§ 1.3); `createContext`
        // and `useContext` are `solid-js`' own and were read in all six of its
        // bundles.
        (
            "@solidjs/signals",
            RC3,
            "runWithOwner",
            CallClaimDomain::Creates,
            RC3_OWNER_CONTEXT_AUDIT,
            "## 1. `runWithOwner` — archive `@solidjs/signals@2.0.0-rc.3`",
            ImplementationVerdict::Closed,
        ),
        (
            "solid-js",
            RC3,
            "createContext",
            CallClaimDomain::Creates,
            RC3_OWNER_CONTEXT_AUDIT,
            "## 2. `createContext` — archive `solid-js@2.0.0-rc.3`",
            ImplementationVerdict::Closed,
        ),
        (
            "solid-js",
            RC3,
            "useContext",
            CallClaimDomain::Creates,
            RC3_OWNER_CONTEXT_AUDIT,
            "## 3. `useContext` — archive `solid-js@2.0.0-rc.3`",
            ImplementationVerdict::Closed,
        ),
        // R6 of § 0, and the C1 consistency flag it forced. Both are the same
        // finding: `semantic-model.md` § creates' [Decision 2026-09-04] makes
        // `ctx.serialize(id, deferred.promise, deferStream)` a `create`, and a
        // flat `(package, export, domain)` row cannot say "except under the
        // browser conditions".
        (
            "solid-js",
            RC3,
            "createSignal",
            CallClaimDomain::Creates,
            RC3_CORE_PRIMITIVES_AUDIT,
            "## 7. `createSignal` — two archives, two rows",
            ImplementationVerdict::Withheld(
                "browser conditions perform no create (§ 7.3); the \
                 node/worker/deno condition's derived overload reaches \
                 ctx.serialize(id, deferred.promise, deferStream) \
                 (dist/server.js:558, :699, :760, :797), which \
                 semantic-model.md § creates' [Decision 2026-09-04] settles is \
                 a create. solid-js re-declares the export \
                 (types/client/hydration.d.ts:246-253) so the \
                 @solidjs/signals row does not cover this import path",
            ),
        ),
        (
            "solid-js",
            RC3,
            "createEffect",
            CallClaimDomain::Creates,
            RC3_CORE_PRIMITIVES_AUDIT,
            "## 7. `createSignal` — two archives, two rows",
            ImplementationVerdict::Withheld(
                "withdrawn 2026-09-04: solid-js.json closes creates: [] from \
                 the browser/development case only, and dist/server.js:868-870 \
                 routes createEffect to serverEffect, which calls \
                 processResult — and so ctx.serialize — whenever the caller \
                 passes options.ssrSource (§ 7.4 C1). The reach is guarded \
                 (node/worker/deno ∧ ctx.async ∧ ssrSource ∈ {server, hybrid} \
                 ∧ a thenable or async-iterable compute result ∧ owner.id ∧ \
                 not NoHydrate) and type-correctly reachable \
                 (types/client/hydration.d.ts:42 augments EffectOptions, :568 \
                 re-declares the export). A flat row carries no guard, so it \
                 is withheld until the table is condition-aware",
            ),
        ),
        // RC6_SIGNALS_AUDIT, 2026-09-25: every `@solidjs/signals` row re-read
        // on rc.6's own bytes, in all three bundles its `exports` map can
        // select. 24 granted; one withheld, below.
        (
            "@solidjs/signals",
            RC6,
            "action",
            CallClaimDomain::Reads,
            RC6_SIGNALS_AUDIT,
            "### 1. `action` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "action",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 2. `action` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "createMemo",
            CallClaimDomain::Reads,
            RC6_SIGNALS_AUDIT,
            "### 1. `createMemo` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "createMemo",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 2. `createMemo` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "createOptimistic",
            CallClaimDomain::Reads,
            RC6_SIGNALS_AUDIT,
            "### 3. `createOptimistic` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "createOptimistic",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 4. `createOptimistic` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "createOptimisticStore",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 6. `createOptimisticStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "createProjection",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 2. `createProjection` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "createRoot",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 3. `createRoot` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "createSignal",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 5. `createSignal` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "createStore",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 1. `createStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "createTrackedEffect",
            CallClaimDomain::Reads,
            RC6_SIGNALS_AUDIT,
            "### 3. `createTrackedEffect` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "createTrackedEffect",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 4. `createTrackedEffect` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "flush",
            CallClaimDomain::Reads,
            RC6_SIGNALS_AUDIT,
            "### 7. `flush` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "flush",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 8. `flush` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "getOwner",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 1. `getOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "onCleanup",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 2. `onCleanup` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "onSettled",
            CallClaimDomain::Reads,
            RC6_SIGNALS_AUDIT,
            "### 5. `onSettled` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "onSettled",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 6. `onSettled` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "reconcile",
            CallClaimDomain::Reads,
            RC6_SIGNALS_AUDIT,
            "### 7. `reconcile` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "reconcile",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 8. `reconcile` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "runWithOwner",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 6. `runWithOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "snapshot",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 3. `snapshot` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "untrack",
            CallClaimDomain::Creates,
            RC6_SIGNALS_AUDIT,
            "### 4. `untrack` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC6,
            "createOptimisticStore",
            CallClaimDomain::Reads,
            RC6_SIGNALS_AUDIT,
            "### 5. `createOptimisticStore` — `reads` — archive `@solidjs/signals@2.0.0-rc.6` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "new in rc.6: the export's own landing router (wrapCommit, \
                 dist/prod/store/next/optimistic.js:202-207) calls stageLanding \
                 -> storeSetterNext(fam.px, draft => stagedApply(draft, ...)), \
                 and stagedApply performs string-key get and ownKeys traps on \
                 fam.px, the store proxy this invocation created, when the \
                 derived computation commits under a retained transaction. \
                 Untracked reads still count under semantic-model.md § reads, \
                 and a flat row cannot carry the condition. rc.3 has none of \
                 this code, and its row stays",
            ),
        ),
        // RC3_SHOW_LOADING_AUDIT, 2026-09-27: rows that rested on a
        // `solid-js.json` summary alone, re-read on rc.3's own runtime bytes
        // and withdrawn there.
        (
            "solid-js",
            RC3,
            "Show",
            CallClaimDomain::Reads,
            RC3_SHOW_LOADING_AUDIT,
            "## 1. `Show` — `reads` — archive `solid-js@2.0.0-rc.3` — **WITHDRAWN**",
            ImplementationVerdict::Withheld(
                "every memo Show creates computes on the call's own stack \
                 (@solidjs/signals setupComputedNode: recompute(e, true) unless \
                 lazy), and two of those computes are Show's own code reading a \
                 memo Show created: createMemo$1(conditionValue, ..) reads \
                 conditionValue(), and the returned memo reads condition() \
                 (dist/solid.js:1153-1174). A read of a source the export \
                 created still counts under semantic-model.md § reads",
            ),
        ),
        (
            "solid-js",
            RC3,
            "Show",
            CallClaimDomain::Creates,
            RC3_SHOW_LOADING_AUDIT,
            "## 2. `Show` — `creates` — archive `solid-js@2.0.0-rc.3` — **WITHDRAWN**",
            ImplementationVerdict::Withheld(
                "the node/worker/deno body (dist/server.js:1853-1872) calls the \
                 server createMemo(() => props.when), whose processResult runs \
                 ctx.serialize(id, deferred.promise, deferStream) when when is a \
                 thenable, ctx.async, the owner has an id and no NoHydrate \
                 (:555-558) -- a create under semantic-model.md § creates' \
                 [Decision 2026-09-04], type-correctly reachable (when: T is \
                 unconstrained). A flat row carries no guard",
            ),
        ),
        (
            "solid-js",
            RC3,
            "Loading",
            CallClaimDomain::Creates,
            RC3_SHOW_LOADING_AUDIT,
            "## 3. `Loading` — `creates` — archive `solid-js@2.0.0-rc.3` — **WITHDRAWN**",
            ImplementationVerdict::Withheld(
                "the node/worker/deno body (dist/server.js:1917-1919) runs \
                 ssrLoadingBoundary under any SSR context, which calls \
                 ctx.serialize(id, \"$$f\") (:1757, :1771, :1816), \
                 ctx.serialize(id + \"_assets\", ..) (:1691) and \
                 ctx.registerFragment(id, ..) (:1779) when its children are \
                 pending -- registrations the per-request render runtime \
                 writes into the response, a create under semantic-model.md \
                 § creates' [Decision 2026-09-04]. A flat row carries no guard",
            ),
        ),
        (
            "@solidjs/web",
            RC3,
            "render",
            CallClaimDomain::Reads,
            RC3_SHOW_LOADING_AUDIT,
            "## 4. `render` — `reads` — archive `@solidjs/web@2.0.0-rc.3` — **WITHDRAWN**",
            ImplementationVerdict::Withheld(
                "render spreads options.insertOptions into insert's options \
                 (dist/web.js:347-378), and effect (:59-65) makes the effect \
                 non-transparent for scope: true; under hydration solid-js' \
                 hydratedEffect (solid.js:645-662) then runs withHydrationGate \
                 for ssrSource: \"client\", which creates a signal that a \
                 compute it authored reads when the effect computes on \
                 render's own stack. The option is undeclared but reaches \
                 render through any non-fresh object",
            ),
        ),
        (
            "@solidjs/web",
            RC3,
            "hydrate",
            CallClaimDomain::Reads,
            RC3_SHOW_LOADING_AUDIT,
            "## 5. `hydrate` — `reads` — archive `@solidjs/web@2.0.0-rc.3` — **WITHDRAWN**",
            ImplementationVerdict::Withheld(
                "hydrate (dist/web.js:1159-1230) calls enableHydration() and \
                 returns render(code, element, [...childNodes], options) with \
                 the caller's own options on every path, so options.insertOptions \
                 reaches the hydration gate read that withdrew render's row \
                 (§ 4); here the hydrating precondition is hydrate's own act",
            ),
        ),
        // RC9_SIGNALS_AUDIT: the five `creates` rows the rc.9 vocabulary
        // review called cheap, each read on rc.9's own bytes in all three
        // builds its `exports` map can select. All five granted; nothing else
        // of rc.9 was read, so nothing else is derivable for rc.9.
        (
            "@solidjs/signals",
            RC9,
            "createRoot",
            CallClaimDomain::Creates,
            RC9_SIGNALS_AUDIT,
            "### 3. `createRoot` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "getOwner",
            CallClaimDomain::Creates,
            RC9_SIGNALS_AUDIT,
            "### 1. `getOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "onCleanup",
            CallClaimDomain::Creates,
            RC9_SIGNALS_AUDIT,
            "### 2. `onCleanup` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "runWithOwner",
            CallClaimDomain::Creates,
            RC9_SIGNALS_AUDIT,
            "### 5. `runWithOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "untrack",
            CallClaimDomain::Creates,
            RC9_SIGNALS_AUDIT,
            "### 4. `untrack` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        // RC9_PARITY_AUDIT (2026-09-27): every other rc.3 `@solidjs/signals`
        // row, read on rc.9's bytes in the same three builds. 19 granted;
        // `createOptimisticStore` `reads` withheld for the rc.6 reason.
        (
            "@solidjs/signals",
            RC9,
            "action",
            CallClaimDomain::Creates,
            RC9_PARITY_AUDIT,
            "### 2. `action` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "action",
            CallClaimDomain::Reads,
            RC9_PARITY_AUDIT,
            "### 1. `action` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "createMemo",
            CallClaimDomain::Creates,
            RC9_PARITY_AUDIT,
            "### 2. `createMemo` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "createMemo",
            CallClaimDomain::Reads,
            RC9_PARITY_AUDIT,
            "### 1. `createMemo` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "createOptimistic",
            CallClaimDomain::Creates,
            RC9_PARITY_AUDIT,
            "### 4. `createOptimistic` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "createOptimistic",
            CallClaimDomain::Reads,
            RC9_PARITY_AUDIT,
            "### 3. `createOptimistic` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "createOptimisticStore",
            CallClaimDomain::Creates,
            RC9_PARITY_AUDIT,
            "### 6. `createOptimisticStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "createOptimisticStore",
            CallClaimDomain::Reads,
            RC9_PARITY_AUDIT,
            "### 5. `createOptimisticStore` — `reads` — archive `@solidjs/signals@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "unchanged from rc.6: the export's own landing router (wrapCommit, \
                 dist/prod/store/next/optimistic.js:194-199, dist/dev.js:7690-7695) \
                 calls stageLanding -> storeSetterNext(fam.px, draft => \
                 stagedApply(draft, ...)), and stagedApply performs string-key get, \
                 length and ownKeys traps on fam.px, the store proxy this invocation \
                 created, when its derived computation commits under a retaining \
                 transaction. Untracked reads still count under semantic-model.md \
                 § reads, and a flat row cannot carry the condition",
            ),
        ),
        (
            "@solidjs/signals",
            RC9,
            "createProjection",
            CallClaimDomain::Creates,
            RC9_PARITY_AUDIT,
            "### 2. `createProjection` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "createSignal",
            CallClaimDomain::Creates,
            RC9_PARITY_AUDIT,
            "### 1. `createSignal` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "createStore",
            CallClaimDomain::Creates,
            RC9_PARITY_AUDIT,
            "### 1. `createStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "createTrackedEffect",
            CallClaimDomain::Creates,
            RC9_PARITY_AUDIT,
            "### 4. `createTrackedEffect` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "createTrackedEffect",
            CallClaimDomain::Reads,
            RC9_PARITY_AUDIT,
            "### 3. `createTrackedEffect` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "flush",
            CallClaimDomain::Creates,
            RC9_PARITY_AUDIT,
            "### 8. `flush` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "flush",
            CallClaimDomain::Reads,
            RC9_PARITY_AUDIT,
            "### 7. `flush` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "onSettled",
            CallClaimDomain::Creates,
            RC9_PARITY_AUDIT,
            "### 6. `onSettled` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "onSettled",
            CallClaimDomain::Reads,
            RC9_PARITY_AUDIT,
            "### 5. `onSettled` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "reconcile",
            CallClaimDomain::Creates,
            RC9_PARITY_AUDIT,
            "### 8. `reconcile` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "reconcile",
            CallClaimDomain::Reads,
            RC9_PARITY_AUDIT,
            "### 7. `reconcile` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "snapshot",
            CallClaimDomain::Creates,
            RC9_PARITY_AUDIT,
            "### 3. `snapshot` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        // RC9_CORE_WEB_AUDIT, 2026-09-27: the 26 rc.3 `solid-js`/`@solidjs/web`
        // rows re-read on rc.9's own bytes, in all six builds of each package.
        (
            "solid-js",
            RC9,
            "For",
            CallClaimDomain::Reads,
            RC9_CORE_WEB_AUDIT,
            "### 1. `For` — `reads` — archive `solid-js@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "solid-js",
            RC9,
            "For",
            CallClaimDomain::Creates,
            RC9_CORE_WEB_AUDIT,
            "### 2. `For` — `creates` — archive `solid-js@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "solid-js",
            RC9,
            "Repeat",
            CallClaimDomain::Reads,
            RC9_CORE_WEB_AUDIT,
            "### 3. `Repeat` — `reads` — archive `solid-js@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "solid-js",
            RC9,
            "Repeat",
            CallClaimDomain::Creates,
            RC9_CORE_WEB_AUDIT,
            "### 4. `Repeat` — `creates` — archive `solid-js@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "solid-js",
            RC9,
            "Match",
            CallClaimDomain::Reads,
            RC9_CORE_WEB_AUDIT,
            "### 5. `Match` — `reads` — archive `solid-js@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "solid-js",
            RC9,
            "Match",
            CallClaimDomain::Creates,
            RC9_CORE_WEB_AUDIT,
            "### 6. `Match` — `creates` — archive `solid-js@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "solid-js",
            RC9,
            "createContext",
            CallClaimDomain::Creates,
            RC9_CORE_WEB_AUDIT,
            "### 10. `createContext` — `creates` — archive `solid-js@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "solid-js",
            RC9,
            "useContext",
            CallClaimDomain::Creates,
            RC9_CORE_WEB_AUDIT,
            "### 11. `useContext` — `creates` — archive `solid-js@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/web",
            RC9,
            "clientOnly",
            CallClaimDomain::Reads,
            RC9_CORE_WEB_AUDIT,
            "### 15. `clientOnly` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/web",
            RC9,
            "clientOnly",
            CallClaimDomain::Creates,
            RC9_CORE_WEB_AUDIT,
            "### 16. `clientOnly` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/web",
            RC9,
            "httpHeader",
            CallClaimDomain::Reads,
            RC9_CORE_WEB_AUDIT,
            "### 17. `httpHeader` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/web",
            RC9,
            "httpHeader",
            CallClaimDomain::Creates,
            RC9_CORE_WEB_AUDIT,
            "### 18. `httpHeader` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/web",
            RC9,
            "httpStatus",
            CallClaimDomain::Reads,
            RC9_CORE_WEB_AUDIT,
            "### 19. `httpStatus` — `reads` — archive `@solidjs/web@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/web",
            RC9,
            "httpStatus",
            CallClaimDomain::Creates,
            RC9_CORE_WEB_AUDIT,
            "### 20. `httpStatus` — `creates` — archive `@solidjs/web@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "solid-js",
            RC9,
            "Show",
            CallClaimDomain::Reads,
            RC9_CORE_WEB_AUDIT,
            "### 7. `Show` — `reads` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "every memo Show creates computes on the call's stack, and Show's own \
                 computes read conditionValue() and condition(), memos Show \
                 created (dist/solid.js:1185-1206, byte-identical to rc.3); a read \
                 of a source the export created still counts",
            ),
        ),
        (
            "solid-js",
            RC9,
            "Show",
            CallClaimDomain::Creates,
            RC9_CORE_WEB_AUDIT,
            "### 8. `Show` — `creates` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "the node/worker/deno body's own createMemo(() => props.when) reaches \
                 ctx.serialize(id, deferred.promise, ..) for a thenable when \
                 (dist/server.js:571-574) -- a create under [Decision 2026-09-04], \
                 type-correctly reachable; a flat row carries no guard",
            ),
        ),
        (
            "solid-js",
            RC9,
            "Loading",
            CallClaimDomain::Creates,
            RC9_CORE_WEB_AUDIT,
            "### 9. `Loading` — `creates` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "the node/worker/deno body's ssrLoadingBoundary calls \
                 ctx.serialize(id, \"$$f\") and ctx.registerFragment(id, ..) when its \
                 children are pending (dist/server.js:2008-2071) -- registrations \
                 the per-request render runtime writes into the response",
            ),
        ),
        (
            "solid-js",
            RC9,
            "createSignal",
            CallClaimDomain::Creates,
            RC9_CORE_WEB_AUDIT,
            "### 12. `createSignal` — `creates` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "browser conditions perform no create (§ 13); the node/worker/deno \
                 derived overload's createMemo reaches ctx.serialize through \
                 processResult (dist/server.js:574, :715, :776, :813), a create \
                 under [Decision 2026-09-04]; a flat row carries no guard",
            ),
        ),
        (
            "solid-js",
            RC9,
            "affects",
            CallClaimDomain::Reads,
            RC9_CORE_WEB_AUDIT,
            "### 14. `affects`, `isPending`, `latest`, `refresh` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "the declaration (types/index.d.ts:1) and the three client builds' body \
                 (dist/solid*.js:2) are @solidjs/signals@2.0.0-rc.9's re-export, so \
                 the census and the proposal side bind the signals archive, and \
                 a solid-js row could cite only the server bodies of what it \
                 denies; the row belongs to @solidjs/signals, paired with these \
                 clean server bodies (server.js:1573-1595)",
            ),
        ),
        (
            "solid-js",
            RC9,
            "affects",
            CallClaimDomain::Creates,
            RC9_CORE_WEB_AUDIT,
            "### 14. `affects`, `isPending`, `latest`, `refresh` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "the declaration (types/index.d.ts:1) and the three client builds' body \
                 (dist/solid*.js:2) are @solidjs/signals@2.0.0-rc.9's re-export, so \
                 the census and the proposal side bind the signals archive, and \
                 a solid-js row could cite only the server bodies of what it \
                 denies; the row belongs to @solidjs/signals, paired with these \
                 clean server bodies (server.js:1573-1595)",
            ),
        ),
        (
            "solid-js",
            RC9,
            "isPending",
            CallClaimDomain::Creates,
            RC9_CORE_WEB_AUDIT,
            "### 14. `affects`, `isPending`, `latest`, `refresh` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "the declaration (types/index.d.ts:1) and the three client builds' body \
                 (dist/solid*.js:2) are @solidjs/signals@2.0.0-rc.9's re-export, so \
                 the census and the proposal side bind the signals archive, and \
                 a solid-js row could cite only the server bodies of what it \
                 denies; the row belongs to @solidjs/signals, paired with these \
                 clean server bodies (server.js:1573-1595)",
            ),
        ),
        (
            "solid-js",
            RC9,
            "latest",
            CallClaimDomain::Creates,
            RC9_CORE_WEB_AUDIT,
            "### 14. `affects`, `isPending`, `latest`, `refresh` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "the declaration (types/index.d.ts:1) and the three client builds' body \
                 (dist/solid*.js:2) are @solidjs/signals@2.0.0-rc.9's re-export, so \
                 the census and the proposal side bind the signals archive, and \
                 a solid-js row could cite only the server bodies of what it \
                 denies; the row belongs to @solidjs/signals, paired with these \
                 clean server bodies (server.js:1573-1595)",
            ),
        ),
        (
            "solid-js",
            RC9,
            "refresh",
            CallClaimDomain::Reads,
            RC9_CORE_WEB_AUDIT,
            "### 14. `affects`, `isPending`, `latest`, `refresh` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "the declaration (types/index.d.ts:1) and the three client builds' body \
                 (dist/solid*.js:2) are @solidjs/signals@2.0.0-rc.9's re-export, so \
                 the census and the proposal side bind the signals archive, and \
                 a solid-js row could cite only the server bodies of what it \
                 denies; the row belongs to @solidjs/signals, paired with these \
                 clean server bodies (server.js:1573-1595)",
            ),
        ),
        (
            "solid-js",
            RC9,
            "refresh",
            CallClaimDomain::Creates,
            RC9_CORE_WEB_AUDIT,
            "### 14. `affects`, `isPending`, `latest`, `refresh` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "the declaration (types/index.d.ts:1) and the three client builds' body \
                 (dist/solid*.js:2) are @solidjs/signals@2.0.0-rc.9's re-export, so \
                 the census and the proposal side bind the signals archive, and \
                 a solid-js row could cite only the server bodies of what it \
                 denies; the row belongs to @solidjs/signals, paired with these \
                 clean server bodies (server.js:1573-1595)",
            ),
        ),
        (
            "@solidjs/web",
            RC9,
            "hydrate",
            CallClaimDomain::Reads,
            RC9_CORE_WEB_AUDIT,
            "### 21. `hydrate` — `reads` — archive `@solidjs/web@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "hydrate turns hydration on and hands the caller's options to render \
                 (dist/web.js:1323-1407), reaching render's hydration-gate read \
                 (§ 22)",
            ),
        ),
        (
            "@solidjs/web",
            RC9,
            "render",
            CallClaimDomain::Reads,
            RC9_CORE_WEB_AUDIT,
            "### 22. `render` — `reads` — archive `@solidjs/web@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "under hydration an undeclared, type-reachable options.insertOptions \
                 ({ scope: true, ssrSource: \"client\" }) makes solid-js' \
                 hydratedEffect create a gate signal that a compute in render's \
                 closure reads on its own stack (dist/web.js:368-400, :61-68; \
                 solid.js:563-570, :665-682)",
            ),
        ),
        // RC9_MERGE_OMIT_MEMO_AUDIT, 2026-09-28: `creates` only. `omit` is
        // closed for every condition; `merge` and the flat `solid-js`
        // `createMemo` are withheld for the server bodies below, and the latter
        // is narrowed by a `browser`-scoped row.
        (
            "@solidjs/signals",
            RC9,
            "omit",
            CallClaimDomain::Creates,
            RC9_MERGE_OMIT_MEMO_AUDIT,
            "## 1. `omit` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            RC9,
            "merge",
            CallClaimDomain::Creates,
            RC9_MERGE_OMIT_MEMO_AUDIT,
            "## 2. `merge` — `creates` — archive `@solidjs/signals@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "the archive's own bodies create nothing, but a solid-js import binds \
                 this declaration (types/index.d.ts:1) while solid-js' node/worker/deno \
                 builds run their own merge (server.js:1222-1229), which wraps every \
                 function source in the server createMemo, whose processResult reaches \
                 ctx.serialize(id, deferred.promise, deferStream) (server.js:574, :715); \
                 a guarded reach still counts, and no row scope can yet say which \
                 solid-js build the import runs",
            ),
        ),
        (
            "solid-js",
            RC9,
            "createMemo",
            CallClaimDomain::Creates,
            RC9_MERGE_OMIT_MEMO_AUDIT,
            "## 3. `createMemo` — `creates` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "the server createMemo (server.js:321-410) runs its compute at creation \
                 and hands a thenable result to processResult, which reaches \
                 ctx.serialize (server.js:574) under node/worker/deno ∧ renderToStream \
                 ∧ an owner id ∧ not NoHydrate; the browser bodies create nothing, and \
                 the scoped row states exactly that (§ 4)",
            ),
        ),
        // RC9_SERVER_BUILDS_AUDIT, 2026-09-28: the server bodies read as the
        // subject of a `node` row. Its `creates` sections withhold the tuples
        // already withheld above (so they are not repeated here); its `reads`
        // sections withhold the two flat `reads` rows.
        (
            "solid-js",
            RC9,
            "createSignal",
            CallClaimDomain::Reads,
            RC9_SERVER_BUILDS_AUDIT,
            "## 3. `createSignal` — `reads` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "under hydration with ssrSource client or hybrid, the browser builds' \
                 hydrateSignalLike hands coreFn an export-authored compute that reads \
                 hydrated(), the signal withHydrationGate created on the same call, and \
                 signals' createMemo runs it at creation (solid.js:563-595, measured); \
                 the server bodies read nothing of their own, but a node row would \
                 need a premise pinning the signals archive whose NotReadyError, \
                 NoOwnerError and ContextNotFoundError the closure constructs, and a \
                 delegate-less host-target scope cannot state one",
            ),
        ),
        (
            "solid-js",
            RC9,
            "createMemo",
            CallClaimDomain::Reads,
            RC9_SERVER_BUILDS_AUDIT,
            "## 4. `createMemo` — `reads` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**",
            ImplementationVerdict::Withheld(
                "hydratedCreateMemo (solid.js:596-601) enters the same hydrateSignalLike \
                 whenever hydrating and not transparent, so the export's own gate compute \
                 reads the signal it created on the call's stack (§ 3.2); the server \
                 bodies are clean, and a node row is unbindable for § 3.3's reason",
            ),
        ),
    ];

    /// The directory under `benchmarks/package-contract-v2/phase0/` that
    /// carries an archive's pinned per-file manifest, as
    /// `<release>/<archive>`.
    ///
    /// Keyed by name **and** version, so two prereleases of one package can
    /// never share a pin (or, through [`audited_slice_path`], a slice).
    /// Deliberately a total match with no fallback: an archive nobody pinned
    /// cannot carry an [`AuditedCitation::Implementation`] at all, because
    /// there would be nothing to check the cited file's digest against.
    fn phase0_archive_directory(package: &str, version: &str) -> &'static str {
        match (package, version) {
            ("@solidjs/signals", RC3) => "rc3/solidjs-signals",
            ("@solidjs/web", RC3) => "rc3/solidjs-web",
            ("solid-js", RC3) => "rc3/solid-js",
            ("@solidjs/signals", RC6) => "rc6/solidjs-signals",
            ("@solidjs/signals", RC9) => "rc9/solidjs-signals",
            ("@solidjs/web", RC9) => "rc9/solidjs-web",
            ("solid-js", RC9) => "rc9/solid-js",
            (other, version) => {
                panic!("no pinned phase0 manifest directory for {other}@{version}")
            }
        }
    }

    /// The environment variable naming an installed tree of the archive's
    /// release (`<root>/<package>/<archive_path>`), for the archive-reading arm
    /// of the citation test, and whether a verification run must set it.
    ///
    /// Every archive `audited-archives.json` lists is provisioned by
    /// `make audited-archives-provision` (`scripts/audited-archives.mjs`),
    /// apart from the tsc-oracle install, and `make test-rust` and
    /// `scripts/verify.sh` arm all three, so `SOLID_CHECKER_EXPECT_PROBE_PINS=1`
    /// makes any one's absence a failure. Without that variable a run that
    /// sets none still performs the unconditional checks: the checked-in slice
    /// hashes to the cited digest, and the cited file is the one pinned in
    /// `phase0/<release>/*/files.json` at that digest and length.
    fn archive_root_variable(version: &str) -> (&'static str, bool) {
        match version {
            RC3 => ("SOLID_CHECKER_RC3_ARCHIVE_ROOT", true),
            RC6 => ("SOLID_CHECKER_RC6_ARCHIVE_ROOT", true),
            RC9 => ("SOLID_CHECKER_RC9_ARCHIVE_ROOT", true),
            other => panic!("no archive-root variable for {other}"),
        }
    }

    /// The version whose bytes each audit document read. An
    /// [`AuditedCitation::Implementation`] may cite an audit only on a row of
    /// that version, so an rc.3 reading can never be cited as an rc.6 one.
    fn audit_version(audit: &str) -> &'static str {
        match audit {
            RC3_CORE_PRIMITIVES_AUDIT | RC3_OWNER_CONTEXT_AUDIT | RC3_SHOW_LOADING_AUDIT => RC3,
            RC6_SIGNALS_AUDIT => RC6,
            RC9_SIGNALS_AUDIT
            | RC9_PARITY_AUDIT
            | RC9_CORE_WEB_AUDIT
            | RC9_MERGE_OMIT_MEMO_AUDIT
            | RC9_SERVER_BUILDS_AUDIT => RC9,
            other => panic!("{other} is not a known audit document"),
        }
    }

    /// Exports an archive defines under another local name and re-exports.
    /// The slice check below accepts the local definition for exactly these,
    /// with the re-export sites cited, rather than loosening for every row.
    ///
    /// rc.6: `export { createProjectionNext as createProjection }` and
    /// `export { createOptimisticStoreNext as createOptimisticStore }`
    /// (`dist/prod/index.js:35,37`, `dist/dev.js:12931,12933`), and
    /// `exports.createProjection = createProjectionNext` /
    /// `exports.createOptimisticStore = createOptimisticStoreNext`
    /// (`dist/node.cjs:11021,11025`).
    ///
    /// rc.9: the same two `export { … as … }` forms in every build
    /// (`dist/prod/index.js:41,43`, `dist/observe/index.js:43,45`,
    /// `dist/dev.js:9498,9500`).
    const DEFINITION_ALIASES: &[(&str, &str, &str, &str)] = &[
        (
            "@solidjs/signals",
            RC6,
            "createOptimisticStore",
            "createOptimisticStoreNext",
        ),
        (
            "@solidjs/signals",
            RC6,
            "createProjection",
            "createProjectionNext",
        ),
        (
            "@solidjs/signals",
            RC9,
            "createOptimisticStore",
            "createOptimisticStoreNext",
        ),
        (
            "@solidjs/signals",
            RC9,
            "createProjection",
            "createProjectionNext",
        ),
    ];

    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(bytes))
    }

    /// The checked-in copy of one `Implementation` citation's byte range, as a
    /// repository-relative path derived from the citation's own fields.
    ///
    /// Derived rather than stored, so a row and its slice file cannot disagree
    /// about which bytes they are: there is no second field to get wrong.
    fn audited_slice_path(
        package: &str,
        version: &str,
        archive_path: &str,
        start_byte: usize,
        end_byte: usize,
    ) -> String {
        format!(
            "rust/crates/solid-dialect/audited-slices/solid-v2/{}/{archive_path}.{start_byte}-{end_byte}.slice",
            phase0_archive_directory(package, version)
        )
    }

    /// Every row cites bytes that say what the row says.
    ///
    /// This is the test that keeps the table from drifting, and it does two
    /// materially different things depending on the citation kind:
    ///
    /// - [`AuditedCitation::Summary`]: re-reads the cited byte range out of the
    ///   cited document, parses *that slice* as the summary object, and
    ///   **re-derives the closure**. A row whose citation moved, whose document
    ///   changed, or whose domain is no longer closed there fails here rather
    ///   than in a certification months later.
    /// - [`AuditedCitation::Implementation`]: re-establishes the **subject** of
    ///   a human reading — the audit document and its section still exist, the
    ///   cited file is still the pinned file at the pinned digest, the range is
    ///   inside it, and the range's bytes still hash to what the row claims.
    ///   Nothing here re-derives closure from a JavaScript body, and nothing
    ///   here pretends to.
    #[test]
    fn every_negative_row_citation_resolves_to_the_bytes_it_claims() {
        let root = repository_root();
        assert!(!NEGATIVE_ROWS.is_empty());
        let mut implementation_citations = 0usize;
        for row in NEGATIVE_ROWS {
            assert!(
                !row.citations.is_empty(),
                "{}:{} carries no citation",
                row.package,
                row.export
            );
            for citation in row.citations {
                let &AuditedCitation::Summary {
                    document,
                    summary: summary_id,
                    start_byte,
                    end_byte,
                } = citation
                else {
                    let &AuditedCitation::Implementation {
                        audit,
                        section,
                        archive_path,
                        file_sha256,
                        start_byte,
                        end_byte,
                        slice_sha256,
                    } = citation
                    else {
                        unreachable!()
                    };
                    implementation_citations += 1;

                    // 0. The audit read this row's archive, not a sibling
                    //    prerelease of the same package.
                    assert_eq!(
                        audit_version(audit),
                        row.version,
                        "{}@{}:{} cites {audit}, which read another version's bytes",
                        row.package,
                        row.version,
                        row.export
                    );

                    // 1. The audit document, and the exact section heading that
                    //    decides this row. The reasoning is only reviewable if
                    //    a reader can find it.
                    let audit_text =
                        std::fs::read_to_string(root.join(audit)).unwrap_or_else(|error| {
                            panic!(
                                "{}:{} cites audit {audit}, which cannot be read: {error}",
                                row.package, row.export
                            )
                        });
                    assert!(
                        audit_text.lines().any(|line| line == section),
                        "{}:{} cites section {section:?}, which {audit} does not contain verbatim",
                        row.package,
                        row.export
                    );

                    // 2. The cited runtime file is the pinned one, at the
                    //    pinned digest, and the range fits inside it. This is
                    //    the same pinned manifest the registry-integrity gate
                    //    already trusts; the citation adds no new authority.
                    let manifest_path = format!(
                        "benchmarks/package-contract-v2/phase0/{}/files.json",
                        phase0_archive_directory(row.package, row.version)
                    );
                    let manifest: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(root.join(&manifest_path)).unwrap())
                            .unwrap();
                    let entry = manifest
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|entry| entry["path"].as_str() == Some(archive_path))
                        .unwrap_or_else(|| {
                            panic!(
                                "{}:{} cites {archive_path}, which {manifest_path} does not pin",
                                row.package, row.export
                            )
                        });
                    assert_eq!(
                        entry["sha256"].as_str(),
                        Some(file_sha256),
                        "{}:{} cites a digest for {archive_path} that {manifest_path} disagrees with",
                        row.package,
                        row.export
                    );
                    let file_bytes = entry["bytes"].as_u64().unwrap() as usize;
                    assert!(
                        start_byte < end_byte && end_byte <= file_bytes,
                        "{}:{} cites {start_byte}..{end_byte} of {archive_path}, which is {file_bytes} bytes",
                        row.package,
                        row.export
                    );

                    // 3. The cited bytes themselves, unconditionally, from the
                    //    checked-in slice. Without this the slice digest would
                    //    only ever be checked where the archive happened to be
                    //    installed, and "green" would mean "skipped".
                    let slice_path = audited_slice_path(
                        row.package,
                        row.version,
                        archive_path,
                        start_byte,
                        end_byte,
                    );
                    let slice = std::fs::read(root.join(&slice_path)).unwrap_or_else(|error| {
                        panic!(
                            "{}:{} cites bytes with no checked-in slice at {slice_path}: {error}",
                            row.package, row.export
                        )
                    });
                    assert_eq!(
                        slice.len(),
                        end_byte - start_byte,
                        "{slice_path} is not {start_byte}..{end_byte} long"
                    );
                    assert_eq!(
                        sha256_hex(&slice),
                        slice_sha256,
                        "{}:{}'s checked-in slice {slice_path} does not hash to the cited digest",
                        row.package,
                        row.export
                    );

                    // The slice must be the *export's own* definition, not an
                    // arbitrary range that happens to hash correctly. The
                    // minified bundles carry the tail of the preceding doc
                    // comment on the definition's first line, so a leading
                    // `*/ ` is expected there and nowhere else. An export the
                    // archive defines under a local name and re-exports is
                    // checked against that local name, and only when
                    // DEFINITION_ALIASES says so.
                    let text = String::from_utf8(slice.clone()).unwrap();
                    let body = text.strip_prefix(" */ ").unwrap_or(&text);
                    let defined = DEFINITION_ALIASES
                        .iter()
                        .find(|(package, version, export, _)| {
                            *package == row.package
                                && *version == row.version
                                && *export == row.export
                        })
                        .map_or(row.export, |(_, _, _, local)| *local);
                    assert!(
                        body.starts_with(&format!("function {defined}("))
                            || body.starts_with(&format!("const {defined} = ")),
                        "{slice_path} does not begin with {}'s definition ({defined}): {:?}",
                        row.export,
                        &body[..body.len().min(60)]
                    );

                    // 4. And, when the archive is on disk, that those bytes are
                    //    still the archive's bytes at the cited offsets — the
                    //    one check the checked-in slice cannot make. Skipping
                    //    is allowed, silently skipping under a verification run
                    //    is not — for a release whose tree a verification run
                    //    provisions (`archive_root_variable`).
                    let (variable, required) = archive_root_variable(row.version);
                    match std::env::var(variable) {
                        Ok(archive_root) if !archive_root.is_empty() => {
                            let file = std::path::Path::new(&archive_root)
                                .join(row.package)
                                .join(archive_path);
                            let bytes = std::fs::read(&file).unwrap_or_else(|error| {
                                panic!("{variable} is set, but {}: {error}", file.display())
                            });
                            assert_eq!(
                                sha256_hex(&bytes),
                                file_sha256,
                                "{} is not the pinned {archive_path}",
                                file.display()
                            );
                            assert_eq!(
                                bytes[start_byte..end_byte],
                                slice[..],
                                "{}:{}'s checked-in slice is not {start_byte}..{end_byte} of {}",
                                row.package,
                                row.export,
                                file.display()
                            );
                        }
                        _ if required => assert_ne!(
                            std::env::var("SOLID_CHECKER_EXPECT_PROBE_PINS").as_deref(),
                            Ok("1"),
                            "SOLID_CHECKER_EXPECT_PROBE_PINS=1, but \
                             {variable} is unset: the cited ranges would only be \
                             checked against the checked-in slices, never \
                             against the archive they claim to quote. \
                             make test-rust and scripts/verify.sh export it \
                             from make audited-archives-provision."
                        ),
                        // An archive a verification run need not arm; the
                        // slice and pin checks above are the whole
                        // verification. Every archive is required today.
                        _ => {}
                    }
                    continue;
                };
                let bytes = std::fs::read(root.join(document)).unwrap();
                assert!(
                    start_byte < end_byte && end_byte <= bytes.len(),
                    "{}:{} cites a range outside {document}",
                    row.package,
                    row.export
                );
                let summary: serde_json::Value = serde_json::from_slice(
                    &bytes[start_byte..end_byte],
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "{}:{} cites bytes that are not a summary object in {document}: {error}",
                        row.package, row.export
                    )
                });
                assert!(
                    closes_domain_empty(&summary["call"], row.domain),
                    "{}:{} cites {document} {start_byte}..{end_byte}, which does not close {} empty",
                    row.package,
                    row.export,
                    row.domain.wire_name()
                );

                // The cited range must be the summary the document maps this
                // export to, in the document the citation names. A range that
                // parses and closes the domain is not enough: it has to be
                // *this export's* summary.
                let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(
                    parsed["package"]["name"].as_str(),
                    Some(row.package),
                    "{}:{} cites a document about another package",
                    row.package,
                    row.export
                );
                assert_eq!(
                    parsed["package"]["version"].as_str(),
                    Some(row.version),
                    "{}@{}:{} cites a document about another version",
                    row.package,
                    row.version,
                    row.export
                );
                let mut bound = false;
                for entrypoint in parsed["entrypoints"].as_object().unwrap().values() {
                    for artifact_case in entrypoint["cases"].as_array().into_iter().flatten() {
                        if artifact_case["exports"][row.export].as_str() == Some(summary_id) {
                            bound = true;
                        }
                    }
                }
                assert!(
                    bound,
                    "{} in {document} does not map {} to {summary_id}",
                    row.package, row.export
                );
                assert_eq!(
                    parsed["summaries"][summary_id], summary,
                    "{}:{}'s cited byte range is not summary {summary_id}",
                    row.package, row.export
                );
            }
        }

        // The Implementation arm is new and easy to lose: a refactor that
        // stopped reaching it would leave every check above passing over the
        // summary citations alone. On rc.3: five rows, three bundles each, then
        // (2026-09-23) `runWithOwner`'s three and the six `solid-js` bundles
        // each for `createContext` and `useContext` -- 30 -- plus
        // (2026-09-25) the one `dist/solid.js` citation of the scoped
        // `solid-js` `createSignal` row. On rc.6 (2026-09-25): 24 rows, three
        // bundles each -- 72. On rc.9 (2026-09-26): 5 rows, three builds each
        // (`dist/prod/**`, `dist/dev.js`/`dist/dev-shared.js`,
        // `dist/observe/**`) -- 15; and (2026-09-27) 19 more rows, three
        // builds each -- 57. On rc.9 `solid-js` and `@solidjs/web`
        // (2026-09-27): 14 flat rows, six builds each -- 84 -- and the scoped
        // `createSignal` row's three browser builds. On rc.9 (2026-09-28):
        // `omit`'s three signals builds and the scoped `createMemo` row's three
        // browser builds.
        assert_eq!(
            implementation_citations,
            30 + 1 + 72 + 15 + 57 + 84 + 3 + 3 + 3,
            "the Implementation citation arm did not run over the rows that need it"
        );
    }

    /// The table is exactly what the audited documents support, minus the named
    /// withholdings — derived here rather than trusted.
    ///
    /// Both directions matter. A row nobody can derive is a fabricated
    /// negative claim; a derivable row that is neither shipped nor withheld is
    /// a silent omission, which is harmless for soundness and corrosive for
    /// review, because it makes the table's contents a matter of who edited it
    /// last.
    #[test]
    fn the_negative_table_is_derived_from_the_audited_documents() {
        use std::collections::{BTreeMap, BTreeSet};

        let documents = audited_documents();
        // (package, version, export, domain) -> whether every case closes it
        let mut observed = BTreeMap::<(String, String, String, CallClaimDomain), bool>::new();
        for (_, bytes) in &documents {
            let document: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            let package = document["package"]["name"].as_str().unwrap().to_owned();
            let version = document["package"]["version"].as_str().unwrap().to_owned();
            for entrypoint in document["entrypoints"].as_object().unwrap().values() {
                for artifact_case in entrypoint["cases"].as_array().into_iter().flatten() {
                    for (export, reference) in artifact_case["exports"].as_object().unwrap() {
                        if Solid2
                            .primitive(export)
                            .and_then(|primitive| Solid2.name_of(primitive))
                            != Some(export.as_str())
                        {
                            continue;
                        }
                        let summary = &document["summaries"][reference.as_str().unwrap()];
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
                            let closed = closes_domain_empty(&summary["call"], domain);
                            observed
                                .entry((package.clone(), version.clone(), export.clone(), domain))
                                .and_modify(|all| *all &= closed)
                                .or_insert(closed);
                        }
                    }
                }
            }
        }

        // Only the domains the table actually admits are compared. Adding a
        // domain to the table means widening this set deliberately.
        let admitted: BTreeSet<CallClaimDomain> =
            NEGATIVE_ROWS.iter().map(|row| row.domain).collect();
        assert_eq!(
            admitted,
            BTreeSet::from([CallClaimDomain::Creates, CallClaimDomain::Reads]),
            "a new domain was added to the table without widening this comparison"
        );

        let from_json: BTreeSet<(String, String, String, CallClaimDomain)> = observed
            .into_iter()
            .filter(|((_, _, _, domain), closed)| *closed && admitted.contains(domain))
            .map(|(key, _)| key)
            .collect();

        // The second source. Every entry names an audit document and a section
        // heading, and both must exist — a hand census that cannot be found is
        // not authority, it is an assertion.
        let root = repository_root();
        let mut implementation_closed = BTreeSet::new();
        let mut implementation_withheld = BTreeSet::new();
        for (package, version, export, domain, audit, section, verdict) in IMPLEMENTATION_AUDITED {
            assert!(
                admitted.contains(domain),
                "{package}:{export} is implementation-audited for a domain the table does not admit"
            );
            assert_eq!(
                Solid2
                    .primitive(export)
                    .and_then(|primitive| Solid2.name_of(primitive)),
                Some(*export),
                "{package}:{export} is not a canonical 2.0 primitive spelling"
            );
            assert!(
                AUDITED_ARCHIVES
                    .iter()
                    .any(|archive| archive.name == *package && archive.version == *version),
                "{package}@{version}:{export} names an archive with no audited tuple"
            );
            assert_eq!(
                audit_version(audit),
                *version,
                "{package}@{version}:{export} names {audit}, which read another version"
            );
            let text = std::fs::read_to_string(root.join(audit)).unwrap_or_else(|error| {
                panic!("{package}:{export} names audit {audit}, which cannot be read: {error}")
            });
            assert!(
                text.lines().any(|line| line == *section),
                "{package}:{export} names section {section:?}, which {audit} does not contain \
                 verbatim"
            );
            let key = (
                (*package).to_owned(),
                (*version).to_owned(),
                (*export).to_owned(),
                *domain,
            );
            match verdict {
                ImplementationVerdict::Closed => {
                    assert!(
                        implementation_closed.insert(key),
                        "{package}:{export} twice"
                    )
                }
                ImplementationVerdict::Withheld(reason) => {
                    assert!(
                        !reason.is_empty(),
                        "{package}:{export} withheld with no reason"
                    );
                    assert!(
                        implementation_withheld.insert(key),
                        "{package}:{export} twice"
                    )
                }
            };
        }
        assert!(
            implementation_closed.is_disjoint(&implementation_withheld),
            "an implementation audit both closed and withheld one row"
        );

        // A row may be shipped from either source; a withholding may name
        // anything either source *examined*, including an export a hand census
        // read and refused. Without that second half, withholding
        // `(solid-js, createSignal)` — which no audited document mentions at
        // all — would be unrepresentable, and the omission check below would
        // have to be loosened instead.
        let derivable: BTreeSet<_> = from_json.union(&implementation_closed).cloned().collect();
        let supported: BTreeSet<_> = derivable.union(&implementation_withheld).cloned().collect();
        // Only an `EveryCondition` row is a claim about the whole archive, so
        // only those are compared with what the sources derive for the whole
        // archive. A scoped row is compared separately below: it must stand on
        // exactly the flat row the sources *withheld*, and on a reading that
        // names its condition.
        let shipped: BTreeSet<(String, String, String, CallClaimDomain)> = NEGATIVE_ROWS
            .iter()
            .filter(|row| row.scope == RowScope::EveryCondition)
            .map(|row| {
                (
                    row.package.to_owned(),
                    row.version.to_owned(),
                    row.export.to_owned(),
                    row.domain,
                )
            })
            .collect();
        let withheld: BTreeSet<(String, String, String, CallClaimDomain)> = WITHHELD
            .iter()
            .map(|(package, version, export, domain)| {
                (
                    (*package).to_owned(),
                    (*version).to_owned(),
                    (*export).to_owned(),
                    *domain,
                )
            })
            .collect();

        assert!(
            shipped.is_disjoint(&withheld),
            "a row is both shipped and withheld"
        );
        assert!(
            withheld.is_subset(&supported),
            "a withholding names a row no audited source examined: {:?}",
            withheld.difference(&supported).collect::<Vec<_>>()
        );
        let expected: BTreeSet<_> = derivable.difference(&withheld).cloned().collect();
        assert_eq!(
            shipped, expected,
            "the shipped table is not the derivable table minus the withholdings"
        );

        // 45 closures across the two admitted domains, all of them rc.3 (no
        // bundled document is about rc.6): 25 `creates` and 20 `reads`.
        // Shipped is that, minus the 3 withholdings that are derivable from a
        // document (`hydrate` and `createEffect` for `creates`, `createEffect`
        // for `reads`), plus the 5 the hand implementation census closed, plus
        // the 3 the 2026-09-23 one did -- 50 on rc.3 -- plus the 24 rows the
        // 2026-09-25 rc.6 re-audit granted. `createSignal`'s rc.3 `creates` and
        // `createOptimisticStore`'s rc.6 `reads` withholdings are derivable
        // only from IMPLEMENTATION_AUDITED, so neither is subtracted here. The
        // 2026-09-26 rc.9 audit adds 5 closed `creates` rows and withholds
        // nothing: it read only the five it grants. The 2026-09-27 rc.9 parity
        // audit reads the other 20: 19 closed, and `createOptimisticStore`
        // `reads` withheld as on rc.6 (again derivable only from
        // IMPLEMENTATION_AUDITED). rc.9 then carries what rc.6 does: 24.
        // The 2026-09-27 rc.3 re-reading (RC3_SHOW_LOADING_AUDIT) withdraws
        // 5 document-derivable rows, each listed in WITHHELD and
        // IMPLEMENTATION_AUDITED.
        assert_eq!(from_json.len(), 45);
        // The 2026-09-27 rc.9 `solid-js`/`@solidjs/web` reading closes 14 rows
        // and withholds 12 (the flat `createSignal` among them, which its
        // scoped row narrows).
        // The 2026-09-28 rc.9 `merge`/`omit`/`createMemo` reading closes 1
        // (`omit`) and withholds 2 (`merge`, and the flat `solid-js`
        // `createMemo`, which its scoped row narrows).
        // The 2026-09-28 rc.9 server-builds reading closes nothing and
        // withholds 2 (the flat `solid-js` `createSignal` and `createMemo`
        // `reads`); its `creates` verdicts restate withholdings counted above.
        assert_eq!(implementation_closed.len(), 8 + 24 + 5 + 19 + 14 + 1);
        assert_eq!(implementation_withheld.len(), 4 + 5 + 12 + 2 + 2);
        let on = |version: &str| shipped.iter().filter(|(_, v, _, _)| v == version).count();
        assert_eq!((on(RC3), on(RC6), on(RC9)), (45, 24, 24 + 14 + 1));
        assert_eq!(shipped.len(), 93 + 14 + 1);

        // The scoped rows: each is a flat row both sources withhold, read
        // under one host-target condition in a section HOST_TARGET_READINGS
        // names, and nothing else. 108 flat + 3 scoped = 111 rows.
        let mut scoped = BTreeSet::new();
        for row in NEGATIVE_ROWS {
            let RowScope::HostTarget(scope) = row.scope else {
                continue;
            };
            let key = (
                row.package.to_owned(),
                row.version.to_owned(),
                row.export.to_owned(),
                row.domain,
            );
            assert!(
                withheld.contains(&key) && implementation_withheld.contains(&key),
                "{key:?} is scoped, but the flat row it narrows is not withheld: a scoped row \
                 exists only where a guarded reach made the flat row unsound"
            );
            let reading = HOST_TARGET_READINGS
                .iter()
                .filter(|(package, version, export, domain, ..)| {
                    *package == row.package
                        && *version == row.version
                        && *export == row.export
                        && *domain == row.domain
                })
                .collect::<Vec<_>>();
            let [(_, _, _, _, audit, section, condition)] = reading.as_slice() else {
                panic!("{key:?} is scoped with {} readings, not one", reading.len());
            };
            assert_eq!(*condition, scope.condition, "{key:?} scope condition");
            let text = std::fs::read_to_string(root.join(audit)).unwrap();
            assert!(
                text.lines().any(|line| line == *section),
                "{key:?} names section {section:?}, which {audit} does not contain verbatim"
            );
            for citation in row.citations {
                let AuditedCitation::Implementation {
                    audit: cited,
                    section: cited_section,
                    ..
                } = citation
                else {
                    panic!("{key:?} is scoped, and a scoped row is a reading of runtime bytes");
                };
                assert_eq!((cited, cited_section), (audit, section), "{key:?} citation");
            }
            assert!(scoped.insert(key), "{:?} is scoped twice", row.export);
        }
        assert_eq!(scoped.len(), 3);
        assert_eq!(NEGATIVE_ROWS.len(), 108 + 3);

        // The two authorities must not be confusable from the row alone: a row
        // the hand census closed cites runtime bytes, and every other row cites
        // a summary. Otherwise an Implementation citation could ship with no
        // entry above, escaping every check in the loop.
        for row in NEGATIVE_ROWS {
            let key = (
                row.package.to_owned(),
                row.version.to_owned(),
                row.export.to_owned(),
                row.domain,
            );
            let cites_implementation = row
                .citations
                .iter()
                .any(|citation| matches!(citation, AuditedCitation::Implementation { .. }));
            let cites_summary = row
                .citations
                .iter()
                .any(|citation| matches!(citation, AuditedCitation::Summary { .. }));
            assert_eq!(
                cites_implementation,
                implementation_closed.contains(&key) || scoped.contains(&key),
                "{key:?}'s citation kind disagrees with IMPLEMENTATION_AUDITED"
            );
            assert!(
                cites_implementation != cites_summary,
                "{key:?} mixes citation kinds; a row has one authority"
            );
        }
    }

    /// `solid-js@2.0.0-rc.9`'s own `createSignal` and `createMemo` carry no
    /// `node` row and no host-free row in either domain
    /// (`RC9_SERVER_BUILDS_AUDIT`): the server builds' `creates` reaches
    /// `ctx.serialize` under `renderToStream`, and host-free `reads` falls on
    /// the browser builds' gate read. Pinned by answer, so a future row that
    /// narrows either export to `node` has to come through that reading.
    #[test]
    fn rc9_solid_js_signal_and_memo_carry_no_server_row() {
        use std::collections::BTreeSet;

        let set = |conditions: &[&str]| {
            conditions
                .iter()
                .map(|condition| (*condition).to_owned())
                .collect::<BTreeSet<_>>()
        };
        let solid_js = archive("solid-js", RC9);
        let authority = Solid2.negative_claim_authority();
        for export in ["createSignal", "createMemo"] {
            for domain in [CallClaimDomain::Creates, CallClaimDomain::Reads] {
                assert!(
                    !authority.denies(solid_js, export, domain),
                    "{export} {domain:?}"
                );
                assert!(
                    !crate::primitive_performs_no_operation(solid_js, export, domain),
                    "{export} {domain:?}"
                );
                for conditions in [
                    set(&[]),
                    set(&["import"]),
                    set(&["import", "node"]),
                    set(&["development", "import", "node"]),
                    set(&["import", "node", "observe"]),
                ] {
                    assert!(
                        !crate::some_audit_denies_primitive(
                            "solid-js",
                            export,
                            domain,
                            &conditions
                        ),
                        "{export} {domain:?} proposed under {conditions:?}"
                    );
                }
            }
            // `reads` has no scoped row at all; `creates` has exactly the
            // `browser` one, which lists no server file.
            assert!(crate::host_target_row(solid_js, export, CallClaimDomain::Reads).is_none());
            let scope = crate::host_target_row(solid_js, export, CallClaimDomain::Creates)
                .unwrap_or_else(|| panic!("{export} keeps its browser-scoped creates row"));
            assert_eq!(scope.condition, HostTargetCondition::Browser);
            assert!(
                scope
                    .runtime
                    .iter()
                    .all(|file| !file.starts_with("dist/server")),
                "{export}'s scoped row lists a server build: {:?}",
                scope.runtime
            );
        }
    }

    /// `render` publishes a `create`, so it has no row — and neither does any
    /// export the audit leaves open. The withheld `hydrate` row is pinned in
    /// the same place, because a future re-audit that corrects `hydrate` has to
    /// come through here.
    #[test]
    fn exports_that_publish_the_operation_or_are_withheld_stay_silent() {
        let web = archive("@solidjs/web", RC3);
        assert!(
            !Solid2
                .negative_claim_authority()
                .denies(web, "render", CallClaimDomain::Creates)
        );
        assert!(!Solid2.negative_claim_authority().denies(
            web,
            "hydrate",
            CallClaimDomain::Creates
        ));
        assert!(Solid2.negative_claim_authority().denies(
            web,
            "clientOnly",
            CallClaimDomain::Creates
        ));

        // The two `solid-js` server-condition withholdings, pinned by answer
        // and not only by list membership. `createEffect` is the row this
        // dialect *withdrew* on 2026-09-04: its audited document closes
        // `creates: []`, and the answer here must still be silence.
        let authority = Solid2.negative_claim_authority();
        let solid_js = archive("solid-js", RC3);
        assert!(!authority.denies(solid_js, "createEffect", CallClaimDomain::Creates));
        assert!(!authority.denies(solid_js, "createSignal", CallClaimDomain::Creates));
        assert!(!crate::primitive_performs_no_operation(
            solid_js,
            "createSignal",
            CallClaimDomain::Creates
        ));
        // `createSignal` carries a scoped row, which is not a denial: only the
        // census, replaying its premises, may read it. `createEffect` has none.
        assert!(
            crate::host_target_row(solid_js, "createSignal", CallClaimDomain::Creates).is_some()
        );
        assert!(
            crate::host_target_row(solid_js, "createEffect", CallClaimDomain::Creates).is_none()
        );

        // And the five the hand implementation census closed, which are keyed
        // to `@solidjs/signals` — the archive the declaration resolves into —
        // not to `solid-js`. rc.6 re-granted all five on its own bytes.
        for (export, signals) in [
            "createRoot",
            "createSignal",
            "getOwner",
            "onCleanup",
            "untrack",
        ]
        .into_iter()
        .flat_map(|export| {
            [
                (export, archive("@solidjs/signals", RC3)),
                (export, archive("@solidjs/signals", RC6)),
            ]
        }) {
            assert!(
                authority.denies(signals, export, CallClaimDomain::Creates),
                "@solidjs/signals@{}:{export} lost its implementation-audited row",
                signals.version
            );
            assert!(
                !authority.denies(solid_js, export, CallClaimDomain::Creates),
                "solid-js:{export} answered from @solidjs/signals' row; the row is archive-keyed"
            );
            // And they deny nothing in `reads`, deliberately. The other
            // nineteen `Reads` rows are derived from a summary's own closure by
            // the citation test, and these five have no summary to derive from:
            // `solidjs-signals.json` audits twelve exports and none of these is
            // among them. `RC3_CORE_PRIMITIVES_AUDIT` is the *creates* audit and
            // establishes nothing about reads, so a `Reads` row here would be a
            // conclusion no reading reached -- the exact thing ADR 0007 refuses
            // to smooth over. Adding one needs a new audit of the same three
            // archive slices, reviewed as a reading.
            //
            // The gap has a measured price: `@solid-primitives/utils`'
            // `tryOnCleanup` is `isDev ? … : onCleanup`, and deciding its reads
            // means deciding `onCleanup`'s, which is 37 consumer call sites left
            // degenerate (docs/precision-backlog.md, wall 1b). Certification
            // cannot supply the row either -- `@solidjs/signals` declines every
            // one of its 3,477 closure candidates on
            // `runtime-accessor-installation`, being a Proxy-based store.
            assert!(
                !authority.denies(signals, export, CallClaimDomain::Reads),
                "@solidjs/signals@{}:{export} gained a reads row; \
                 it must cite a reading that establishes reads, not the creates audit",
                signals.version
            );
        }
        // The control for the five negatives above: a `Reads` question is
        // answerable, so their silence is this table's answer and not a domain
        // the authority never says yes to.
        for version in [RC3, RC6] {
            assert!(authority.denies(
                archive("@solidjs/signals", version),
                "onSettled",
                CallClaimDomain::Reads
            ));
        }

        // `render`'s summary really does publish the operation, so the silence
        // above is the audit's answer and not a missing row.
        let root = repository_root();
        let bytes =
            std::fs::read(root.join("pkg/contracts/bundled/solid-v2/solidjs-web.json")).unwrap();
        let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let summary = document["entrypoints"]["."]["cases"][0]["exports"]["render"]
            .as_str()
            .unwrap();
        assert_eq!(
            document["summaries"][summary]["call"]["creates"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    /// An rc.3 row never answers for rc.6's bytes: rc.6 answers exactly from
    /// the rows its own re-audit granted.
    ///
    /// The re-audit re-read every rc.3 `@solidjs/signals` row and added none,
    /// so rc.6's rows are rc.3's minus the one it withheld —
    /// `createOptimisticStore` `reads` — and that row must be denied on rc.3
    /// and silent on rc.6. A tuple carrying rc.6's coordinate over other bytes
    /// (another integrity, another manifest, or rc.3's pair of both) is not an
    /// audited archive and denies nothing.
    #[test]
    fn rc3_rows_do_not_answer_for_rc6_unless_rc6_carries_its_own() {
        use std::collections::BTreeSet;

        let authority = Solid2.negative_claim_authority();
        let rc3 = archive("@solidjs/signals", RC3);
        let rc6 = archive("@solidjs/signals", RC6);
        let rows_of = |version: &str| -> BTreeSet<(&str, CallClaimDomain)> {
            NEGATIVE_ROWS
                .iter()
                .filter(|row| row.package == "@solidjs/signals" && row.version == version)
                .map(|row| (row.export, row.domain))
                .collect()
        };
        let (on_rc3, on_rc6) = (rows_of(RC3), rows_of(RC6));
        assert_eq!((on_rc3.len(), on_rc6.len()), (25, 24));
        assert!(on_rc6.is_subset(&on_rc3), "the rc.6 re-audit added a row");
        assert_eq!(
            on_rc3.difference(&on_rc6).copied().collect::<Vec<_>>(),
            [("createOptimisticStore", CallClaimDomain::Reads)]
        );

        for &(export, domain) in &on_rc3 {
            assert!(
                authority.denies(rc3, export, domain),
                "rc.3 {export} {domain:?}"
            );
            assert_eq!(
                authority.denies(rc6, export, domain),
                on_rc6.contains(&(export, domain)),
                "rc.6 {export} {domain:?} answered from a row it does not carry"
            );
        }
        assert!(authority.denies(rc3, "createOptimisticStore", CallClaimDomain::Reads));
        assert!(!authority.denies(rc6, "createOptimisticStore", CallClaimDomain::Reads));
        for export in ["getOwner", "onCleanup"] {
            assert!(authority.denies(rc6, export, CallClaimDomain::Creates));
        }

        for (why, spliced) in [
            (
                "rc.6's coordinate with rc.3's integrity and manifest",
                AuditedArchive {
                    version: RC6,
                    ..*rc3
                },
            ),
            (
                "rc.6 with another integrity",
                AuditedArchive {
                    integrity: "sha512-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==",
                    ..*rc6
                },
            ),
            (
                "rc.6 with rc.3's manifest digest",
                AuditedArchive {
                    manifest_sha256: rc3.manifest_sha256,
                    ..*rc6
                },
            ),
        ] {
            assert!(
                !authority.denies(&spliced, "getOwner", CallClaimDomain::Creates),
                "{why} must deny nothing"
            );
        }
    }

    /// An export outside the audited document, and one the vocabulary does not
    /// spell canonically, both answer nothing.
    #[test]
    fn unaudited_and_non_canonical_exports_answer_nothing() {
        let authority = Solid2.negative_claim_authority();
        // Audited archive, real 2.0 export, no summary in the document and no
        // hand census either.
        for version in [RC3, RC6] {
            assert!(!authority.denies(
                archive("@solidjs/signals", version),
                "createContext",
                CallClaimDomain::Creates
            ));
        }
        assert!(!authority.denies(
            archive("solid-js", RC3),
            "createRenderEffect",
            CallClaimDomain::Creates
        ));
        // Audited archive, closed in the document, not a canonical primitive.
        assert!(!authority.denies(
            archive("@solidjs/signals", RC3),
            "isEqual",
            CallClaimDomain::Creates
        ));
        assert!(!authority.denies(
            archive("@solidjs/web", RC3),
            "applyRef",
            CallClaimDomain::Creates
        ));
        // Not an audited archive at all, even with a row-bearing package's
        // name and every other field of an audited tuple.
        let router = AuditedArchive {
            name: "@solidjs/router",
            ..*archive("@solidjs/signals", RC3)
        };
        assert!(!authority.denies(&router, "createMemo", CallClaimDomain::Creates));
        assert!(authority.archives_named("@solidjs/router").next().is_none());
    }

    /// Every archive tuple is checked against the evidence its archive has.
    ///
    /// An rc.3 tuple is the audited document's own `package` block, all four
    /// fields, and every bundled document naming the package agrees. rc.6 has
    /// no bundled document, so its tuple is checked against the pinned phase0
    /// record: the pinned `package.json` names the archive and hashes to
    /// `manifest_sha256`, `files.json` pins that file at the same digest and
    /// length, and `exports.json` is its `exports` map. The integrity is not
    /// checked for rc.6, because nothing here holds the tarball it digests;
    /// see [`AUDITED_ARCHIVES`] for why that cannot bind the rows to other
    /// bytes. rc.9 is checked the same way, and additionally its tuple's
    /// integrity must equal the registry integrity the downloaded tarball was
    /// verified against (`tarball.json`), which is a record of that check,
    /// not a re-derivation here.
    #[test]
    fn audited_archive_tuples_match_their_documents_or_their_pinned_manifest() {
        #[derive(Clone, Copy)]
        enum Evidence {
            BundledDocuments,
            PinnedManifest,
        }
        let evidence = |archive: &AuditedArchive| match (archive.name, archive.version) {
            (_, RC3) => Evidence::BundledDocuments,
            ("@solidjs/signals", RC6 | RC9) | ("@solidjs/web" | "solid-js", RC9) => {
                Evidence::PinnedManifest
            }
            (name, version) => panic!("{name}@{version} has no stated evidence"),
        };

        let root = repository_root();
        let documents = audited_documents();
        for archive in AUDITED_ARCHIVES {
            let naming = documents
                .iter()
                .filter_map(|(path, bytes)| {
                    let document: serde_json::Value = serde_json::from_slice(bytes).unwrap();
                    (document["package"]["name"].as_str() == Some(archive.name)
                        && document["package"]["version"].as_str() == Some(archive.version))
                    .then_some((path, document))
                })
                .collect::<Vec<_>>();
            match evidence(archive) {
                Evidence::BundledDocuments => {
                    assert!(
                        !naming.is_empty(),
                        "{} has no audited document",
                        archive.name
                    );
                    for (path, document) in naming {
                        assert_eq!(
                            document["package"]["integrity"].as_str(),
                            Some(archive.integrity),
                            "{path} disagrees about {}'s integrity",
                            archive.name
                        );
                        assert_eq!(
                            document["package"]["manifest"]["sha256"].as_str(),
                            Some(archive.manifest_sha256),
                            "{path} disagrees about {}'s manifest digest",
                            archive.name
                        );
                    }
                }
                Evidence::PinnedManifest => {
                    assert!(
                        naming.is_empty(),
                        "{}@{} has a bundled document; check the tuple against it",
                        archive.name,
                        archive.version
                    );
                    let directory = root
                        .join("benchmarks/package-contract-v2/phase0")
                        .join(phase0_archive_directory(archive.name, archive.version));
                    let manifest = std::fs::read(directory.join("package.json")).unwrap();
                    assert_eq!(
                        sha256_hex(&manifest),
                        archive.manifest_sha256,
                        "the pinned package.json of {}@{} is not the tuple's manifest",
                        archive.name,
                        archive.version
                    );
                    let manifest: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
                    assert_eq!(manifest["name"].as_str(), Some(archive.name));
                    assert_eq!(manifest["version"].as_str(), Some(archive.version));
                    let files: serde_json::Value = serde_json::from_slice(
                        &std::fs::read(directory.join("files.json")).unwrap(),
                    )
                    .unwrap();
                    let entry = files
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|entry| entry["path"].as_str() == Some("package.json"))
                        .expect("files.json pins package.json");
                    assert_eq!(entry["sha256"].as_str(), Some(archive.manifest_sha256));
                    assert_eq!(
                        entry["bytes"].as_u64(),
                        Some(std::fs::read(directory.join("package.json")).unwrap().len() as u64)
                    );
                    let exports: serde_json::Value = serde_json::from_slice(
                        &std::fs::read(directory.join("exports.json")).unwrap(),
                    )
                    .unwrap();
                    assert_eq!(
                        exports, manifest["exports"],
                        "exports.json is not the manifest's map"
                    );
                    assert!(archive.integrity.starts_with("sha512-"));
                    // rc.9's tarball was downloaded and verified; rc.6's was
                    // not, so only rc.9 must carry the record.
                    let record = std::fs::read(directory.join("tarball.json"));
                    assert_eq!(
                        record.is_ok(),
                        archive.version == RC9,
                        "{}@{}: a tarball record exists exactly where the tarball was verified",
                        archive.name,
                        archive.version
                    );
                    if let Ok(record) = record {
                        let record: serde_json::Value = serde_json::from_slice(&record).unwrap();
                        assert_eq!(record["name"].as_str(), Some(archive.name));
                        assert_eq!(record["version"].as_str(), Some(archive.version));
                        for field in [
                            &record["registryIntegrity"],
                            &record["downloaded"]["sha512"],
                            &record["lockfileIntegrity"]["integrity"],
                        ] {
                            assert_eq!(
                                field.as_str(),
                                Some(archive.integrity),
                                "{}@{}'s tarball record disagrees with its tuple",
                                archive.name,
                                archive.version
                            );
                        }
                        assert_eq!(
                            record["downloaded"]["extractedFileCount"].as_u64(),
                            Some(files.as_array().unwrap().len() as u64),
                            "files.json does not pin every file of the verified tarball"
                        );
                    }
                }
            }
        }

        // Every audited package in the directory is listed, at its version. An
        // archive read but unlisted would make "never looked" and "found
        // nothing" the same answer, which is the distinction the list exists
        // for.
        for (path, bytes) in &documents {
            let document: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            let name = document["package"]["name"].as_str().unwrap();
            let version = document["package"]["version"].as_str().unwrap();
            assert!(
                AUDITED_ARCHIVES
                    .iter()
                    .any(|archive| archive.name == name && archive.version == version),
                "{path} audits {name}@{version}, which the archive list omits"
            );
        }
    }

    /// Audited archives that deliberately do **not** answer a scoped row's
    /// delegate, as `(package, version, export, domain)`.
    ///
    /// A delegate is bound against the one audited archive of its package in
    /// the certification's closure, and the census refuses the scoped row
    /// when that archive carries no every-condition row denying the delegate
    /// ("whose audit carries no every-condition row denying it",
    /// `contract_certification/type_facts.rs`). So an archive listed here makes
    /// the scoped row fail closed beside it, never bind. Each entry would be an
    /// archive whose reading did not cover the delegate.
    ///
    /// Empty since 2026-09-27: rc.9's one gap (`createSignal` `creates`, which
    /// the five-row `RC9_SIGNALS_AUDIT` did not read) is granted by
    /// `RC9_PARITY_AUDIT`, so every audited `@solidjs/signals` archive answers
    /// both delegates of the scoped `solid-js` `createSignal` row.
    const DELEGATE_GAPS: &[(&str, &str, &str, CallClaimDomain)] = &[];

    /// A scoped row's premises are well formed, and nothing but the census
    /// entry reads it.
    ///
    /// - its runtime list is exactly the set of files its citations read, each
    ///   pinned in the archive's `files.json` — so a file the audit did not
    ///   walk can never be listed without a citation to it;
    /// - every delegate is another archive's canonical primitive that every
    ///   audited archive of that package denies with an `EveryCondition` row,
    ///   except exactly the archives [`DELEGATE_GAPS`] names (beside which the
    ///   census refuses the scoped row) — and at least one archive answers it,
    ///   because a delegate no audited archive answers would make the row
    ///   unbindable, which is a table defect rather than a refusal;
    /// - the proposal side consults it only for a case whose conditions carry
    ///   the scope's condition.
    #[test]
    fn host_target_rows_state_checkable_premises() {
        use std::collections::BTreeSet;

        let root = repository_root();
        let mut seen = 0usize;
        for row in NEGATIVE_ROWS {
            let RowScope::HostTarget(scope) = row.scope else {
                continue;
            };
            seen += 1;
            assert!(!scope.runtime.is_empty(), "{} lists no runtime", row.export);
            let listed = scope.runtime.iter().copied().collect::<BTreeSet<_>>();
            assert_eq!(listed.len(), scope.runtime.len(), "duplicate runtime file");
            let cited = row
                .citations
                .iter()
                .map(|citation| match citation {
                    AuditedCitation::Implementation { archive_path, .. } => *archive_path,
                    AuditedCitation::Summary { .. } => {
                        panic!("{} is scoped and cites a summary", row.export)
                    }
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(
                listed, cited,
                "{}'s runtime list is not the set of files its citations read",
                row.export
            );
            let manifest: serde_json::Value = serde_json::from_slice(
                &std::fs::read(root.join(format!(
                    "benchmarks/package-contract-v2/phase0/{}/files.json",
                    phase0_archive_directory(row.package, row.version)
                )))
                .unwrap(),
            )
            .unwrap();
            for file in scope.runtime {
                assert!(
                    manifest
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|entry| entry["path"].as_str() == Some(*file)),
                    "{file} is not a file of {}@{}",
                    row.package,
                    row.version
                );
            }

            assert!(!scope.delegates.is_empty());
            for &(package, export, domain) in scope.delegates {
                assert_ne!(package, row.package, "a row may not delegate to itself");
                assert_eq!(
                    Solid2
                        .primitive(export)
                        .and_then(|primitive| Solid2.name_of(primitive)),
                    Some(export),
                    "delegate {package}:{export} is not a canonical primitive"
                );
                let archives = AUDITED_ARCHIVES
                    .iter()
                    .filter(|archive| archive.name == package)
                    .collect::<Vec<_>>();
                assert!(!archives.is_empty(), "delegate {package} is not audited");
                let mut answered = 0usize;
                for archive in archives {
                    let gap =
                        DELEGATE_GAPS.contains(&(archive.name, archive.version, export, domain));
                    assert_eq!(
                        crate::primitive_performs_no_operation(archive, export, domain),
                        !gap,
                        "{}@{} delegate {export} {domain:?}: denied exactly where no gap is listed",
                        archive.name,
                        archive.version
                    );
                    answered += usize::from(!gap);
                }
                assert!(
                    answered > 0,
                    "no audited archive answers delegate {package}:{export}"
                );
            }

            // Never a denial, and reachable through the scoped entry alone.
            let audited = archive(row.package, row.version);
            assert!(
                !Solid2
                    .negative_claim_authority()
                    .denies(audited, row.export, row.domain)
            );
            assert_eq!(
                crate::host_target_row(audited, row.export, row.domain),
                Some(&scope)
            );

            // The proposal side reads the case's conditions.
            let set = |conditions: &[&str]| {
                conditions
                    .iter()
                    .map(|condition| (*condition).to_owned())
                    .collect::<BTreeSet<_>>()
            };
            let condition = scope.condition.as_str();
            for (conditions, expected) in [
                (set(&["import"]), false),
                (set(&["import", "solid"]), false),
                (set(&["import", "node"]), false),
                (set(&[]), false),
                (set(&[condition, "import"]), true),
                (set(&[condition, "development", "import"]), true),
            ] {
                assert_eq!(
                    crate::some_audit_denies_primitive(
                        row.package,
                        row.export,
                        row.domain,
                        &conditions
                    ),
                    expected,
                    "{}:{} proposed under {conditions:?}",
                    row.package,
                    row.export
                );
            }
        }
        assert_eq!(seen, 3);

        // An `EveryCondition` row answers the proposal side under any set,
        // the empty one included.
        assert!(crate::some_audit_denies_primitive(
            "@solidjs/signals",
            "createSignal",
            CallClaimDomain::Creates,
            &BTreeSet::new()
        ));
        assert!(
            crate::host_target_row(
                archive("@solidjs/signals", RC6),
                "createSignal",
                CallClaimDomain::Creates
            )
            .is_none(),
            "an EveryCondition row is not a scoped row"
        );
    }

    /// Rows are sorted, unique, keyed to a listed archive, and spelled
    /// canonically.
    #[test]
    fn negative_rows_are_sorted_unique_and_canonical() {
        let mut previous: Option<(&str, &str, &str, CallClaimDomain)> = None;
        for row in NEGATIVE_ROWS {
            let key = (row.package, row.version, row.export, row.domain);
            if let Some(previous) = previous {
                assert!(previous < key, "{key:?} is out of order after {previous:?}");
            }
            previous = Some(key);
            // Exactly one tuple: the row is about one archive's bytes.
            assert_eq!(
                AUDITED_ARCHIVES
                    .iter()
                    .filter(|archive| archive.name == row.package && archive.version == row.version)
                    .count(),
                1,
                "{key:?} does not name exactly one audited archive"
            );
            assert_eq!(
                Solid2
                    .primitive(row.export)
                    .and_then(|primitive| Solid2.name_of(primitive)),
                Some(row.export),
                "{key:?} is not a canonical 2.0 primitive spelling"
            );
        }
    }
}
