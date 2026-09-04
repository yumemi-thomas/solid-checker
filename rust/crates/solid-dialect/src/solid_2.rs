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
    Dialect, DialectNegativeAuthority, Execution, NegativeClaimRow, Primitive, ReactiveRole,
    ResultSlot, TrackedCallbackTiming, Version, lookup, reverse,
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

/// The exact published archives this dialect's negative rows were read
/// against, with every field taken verbatim from the audited document's own
/// `package` block. Byte-checked against those documents by a test, so a
/// re-audit of different bytes cannot leave a stale tuple behind.
///
/// `manifest_sha256` is the digest of the archive's own `package.json`, which
/// a caller re-derives from the authenticated snapshot (`sha256` of
/// `snapshot.read("package.json")`) rather than reading from a manifest a
/// resolver reported. All three were confirmed against the installed rc.3
/// trees.
///
/// `solid-js@2.0.0-rc.3` is listed even though its audited document covers
/// only ten exports: the archive was read, and "read and found nothing to
/// deny about `createSignal`" has to be distinguishable from "never looked".
const AUDITED_ARCHIVES: &[AuditedArchive] = &[
    AuditedArchive {
        name: "@solidjs/signals",
        version: "2.0.0-rc.3",
        integrity: "sha512-/yPhTf3xS1FRR4MX8kTYCd4MjsFxzwkO+KyOTfbu35lTEiaJ4Fxy+JL91XonDzt31GV1mYaZ9CGD2TQIzvXuNA==",
        manifest_sha256: "22d27a9ebdc7b4fbfc65b9857bbea96ea60d3617697fd628b42b6e1253ffdb76",
    },
    AuditedArchive {
        name: "@solidjs/web",
        version: "2.0.0-rc.3",
        integrity: "sha512-5ckKgOjem1pN5ADycOk6TjHmTtjbbN2fukqxo6RW3Oe3H7z0gaXWAdt8dLISto5/O4Nn8VxprFXFWpfy31+DUg==",
        manifest_sha256: "ee9b514b90b06b679d2376c5b5a993c0391aa66ec744e453ec3e534babd30e8e",
    },
    AuditedArchive {
        name: "solid-js",
        version: "2.0.0-rc.3",
        integrity: "sha512-pmW6bRoTvfp/rN4jN7JmLvSaoIpFt7wm0Hi3j508S/smuJqUbRg3dQEjOPTkAwHW+McYnXrMG7cJ4AMNpLevtQ==",
        manifest_sha256: "e703e7986516ac05ee91fdd64897c2d150aea948cb5bf77eae8673da5008ee4b",
    },
];

/// The hand implementation census over the exact published rc.3 bytes of five
/// core `@solidjs/signals` primitives, and the source of every
/// [`AuditedCitation::Implementation`] below.
///
/// Signed off by delegation, 2026-09-04.
const RC3_CORE_PRIMITIVES_AUDIT: &str =
    "docs/package-contract-v2/audits/2026-09-04-solid-2-rc3-core-primitives-creates.md";

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
///
/// # Only `creates`, and only for now
///
/// Every row here is [`CallClaimDomain::Creates`]. The other seven kinded
/// domains are withheld wholesale, because the audited documents' closures in
/// them are not yet admissible as negative authority and each counter-example
/// below is a defect against the *audit*, not against this table:
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
/// - **`reads`, `writes`, `invalidates`, `cleanups`, `disposals`.** No
///   counter-example found, and no positive review performed either. They are
///   silent because nothing here has read them, which is the only honest
///   default: the implementation census needs `creates` first
///   (`phase21/2026-09-03-implementation-census-plan.md` § 4.3) and `reads`
///   second (§ 4.4), and a domain is added when it is audited, not when it is
///   convenient.
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
///   row must be withheld until the table is condition-aware.
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
    NegativeClaimRow {
        package: "@solidjs/signals",
        export: "action",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-c094d35ac3f70f84acaae0d933ed0c4c46071004615d4a1351a11a01bf987552",
            start_byte: 33507,
            end_byte: 38878,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        export: "createMemo",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-6970e6d02d81c014fd7c2ef7aee46716c95cb9aac16a28e9f8adb95ece54eab1",
            start_byte: 12806,
            end_byte: 17314,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        export: "createOptimistic",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-92071bb735b571320a76e500e8f0dc47db0f11df19201d2b3931d2da5d763e37",
            start_byte: 25043,
            end_byte: 27967,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        export: "createOptimisticStore",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-034586f31ead4bb594c03ada1202fb3b439455294d049727cb6f898c65cf5283",
            start_byte: 6985,
            end_byte: 10025,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        export: "createProjection",
        domain: CallClaimDomain::Creates,
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
        export: "createRoot",
        domain: CallClaimDomain::Creates,
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
        export: "createSignal",
        domain: CallClaimDomain::Creates,
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
        export: "createStore",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-7080e21f5c75fdb8ffd32ef595390c4164a07969282c6a843573230ed36de5f5",
            start_byte: 17396,
            end_byte: 23217,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        export: "createTrackedEffect",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-aab0640db7c783a35e1c955cbf19c22197542f3eecb3f89b28433694eb07ff6a",
            start_byte: 29857,
            end_byte: 33425,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        export: "flush",
        domain: CallClaimDomain::Creates,
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
        export: "getOwner",
        domain: CallClaimDomain::Creates,
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
        export: "onCleanup",
        domain: CallClaimDomain::Creates,
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
    NegativeClaimRow {
        package: "@solidjs/signals",
        export: "onSettled",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-5a08fc896d6f18c5378c69bc27d5fc1ddaeb013364aff1421330341801111663",
            start_byte: 10107,
            end_byte: 12724,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        export: "reconcile",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
            summary: "summary-97b25908bde1ce8220884836f97f37a42f6719bcf3b723d9de5746955fcc12dd",
            start_byte: 28049,
            end_byte: 29775,
        }],
    },
    NegativeClaimRow {
        package: "@solidjs/signals",
        export: "snapshot",
        domain: CallClaimDomain::Creates,
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
        export: "untrack",
        domain: CallClaimDomain::Creates,
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
    NegativeClaimRow {
        package: "@solidjs/web",
        export: "clientOnly",
        domain: CallClaimDomain::Creates,
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
        export: "httpHeader",
        domain: CallClaimDomain::Creates,
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
        export: "httpStatus",
        domain: CallClaimDomain::Creates,
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
        package: "solid-js",
        export: "For",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-782061630d49ccfa837915324fb3915d5acaa9393175694c750d975e49e591c5",
            start_byte: 6598,
            end_byte: 12189,
        }],
    },
    NegativeClaimRow {
        package: "solid-js",
        export: "Loading",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-7e8eaca8531ccab3121be0a039c44b383b80aa9d8c0fa51cca31884efc083149",
            start_byte: 12271,
            end_byte: 14398,
        }],
    },
    NegativeClaimRow {
        package: "solid-js",
        export: "Match",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-782061630d49ccfa837915324fb3915d5acaa9393175694c750d975e49e591c5",
            start_byte: 6598,
            end_byte: 12189,
        }],
    },
    NegativeClaimRow {
        package: "solid-js",
        export: "Repeat",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-782061630d49ccfa837915324fb3915d5acaa9393175694c750d975e49e591c5",
            start_byte: 6598,
            end_byte: 12189,
        }],
    },
    NegativeClaimRow {
        package: "solid-js",
        export: "Show",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-782061630d49ccfa837915324fb3915d5acaa9393175694c750d975e49e591c5",
            start_byte: 6598,
            end_byte: 12189,
        }],
    },
    NegativeClaimRow {
        package: "solid-js",
        export: "affects",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-92efacc141c683cc0a3779fa9106ff29f624ed876f3f6d0e0635643dec1d46fd",
            start_byte: 22614,
            end_byte: 24478,
        }],
    },
    NegativeClaimRow {
        package: "solid-js",
        export: "isPending",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-d41bc9d7ea19dd2a6dae7d51199a8448e627d5b6ac99a2eeb20cb02c7799c724",
            start_byte: 24560,
            end_byte: 26351,
        }],
    },
    NegativeClaimRow {
        package: "solid-js",
        export: "latest",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-13d78920672aa68cb9fb09d4b51dafaf4281d67628d73e4ba93f06de39512c40",
            start_byte: 2366,
            end_byte: 4224,
        }],
    },
    NegativeClaimRow {
        package: "solid-js",
        export: "refresh",
        domain: CallClaimDomain::Creates,
        citations: &[AuditedCitation::Summary {
            document: "pkg/contracts/bundled/solid-v2/solid-js.json",
            summary: "summary-49a501fb15bcd7c7961085bd54009503618b3729e6e49e8d299f0eb90ad9d322",
            start_byte: 4306,
            end_byte: 6516,
        }],
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

    /// `pkg/contracts/bundled/solid-v2/solid-js.json`, the artifact
    /// `solid-facts-backend` compiles in for this dialect.
    fn bundled_contract_label(&self) -> &'static str {
        "solid-v2/solid-js.json"
    }

    /// [`AUDITED_ARCHIVES`] and [`NEGATIVE_ROWS`] — 28 `creates` denials, 23
    /// read out of the audited rc.3 contract documents and five out of the
    /// archive's own runtime bytes by hand ([`RC3_CORE_PRIMITIVES_AUDIT`]),
    /// with the withholdings named there.
    fn negative_claim_authority(&self) -> &'static DialectNegativeAuthority {
        &NEGATIVE_AUTHORITY
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
    const WITHHELD: &[(&str, &str, CallClaimDomain)] = &[
        ("@solidjs/web", "hydrate", CallClaimDomain::Creates),
        ("solid-js", "createEffect", CallClaimDomain::Creates),
        ("solid-js", "createSignal", CallClaimDomain::Creates),
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
            "createRoot",
            CallClaimDomain::Creates,
            RC3_CORE_PRIMITIVES_AUDIT,
            "## 3. `createRoot` — archive `@solidjs/signals@2.0.0-rc.3`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            "createSignal",
            CallClaimDomain::Creates,
            RC3_CORE_PRIMITIVES_AUDIT,
            "## 7. `createSignal` — two archives, two rows",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            "getOwner",
            CallClaimDomain::Creates,
            RC3_CORE_PRIMITIVES_AUDIT,
            "## 4. `getOwner` — archive `@solidjs/signals@2.0.0-rc.3`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            "onCleanup",
            CallClaimDomain::Creates,
            RC3_CORE_PRIMITIVES_AUDIT,
            "## 5. `onCleanup` — archive `@solidjs/signals@2.0.0-rc.3`",
            ImplementationVerdict::Closed,
        ),
        (
            "@solidjs/signals",
            "untrack",
            CallClaimDomain::Creates,
            RC3_CORE_PRIMITIVES_AUDIT,
            "## 6. `untrack` — archive `@solidjs/signals@2.0.0-rc.3`",
            ImplementationVerdict::Closed,
        ),
        // R6 of § 0, and the C1 consistency flag it forced. Both are the same
        // finding: `semantic-model.md` § creates' [Decision 2026-09-04] makes
        // `ctx.serialize(id, deferred.promise, deferStream)` a `create`, and a
        // flat `(package, export, domain)` row cannot say "except under the
        // browser conditions".
        (
            "solid-js",
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
    ];

    /// The directory under `benchmarks/package-contract-v2/phase0/rc3/` that
    /// carries an archive's pinned per-file manifest.
    ///
    /// Deliberately a total match with no fallback: an archive nobody pinned
    /// cannot carry an [`AuditedCitation::Implementation`] at all, because
    /// there would be nothing to check the cited file's digest against.
    fn phase0_archive_directory(package: &str) -> &'static str {
        match package {
            "@solidjs/signals" => "solidjs-signals",
            "@solidjs/web" => "solidjs-web",
            "solid-js" => "solid-js",
            other => panic!("no pinned phase0 manifest directory for {other}"),
        }
    }

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
        archive_path: &str,
        start_byte: usize,
        end_byte: usize,
    ) -> String {
        format!(
            "rust/crates/solid-dialect/audited-slices/solid-v2/{}/{archive_path}.{start_byte}-{end_byte}.slice",
            phase0_archive_directory(package)
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
                        "benchmarks/package-contract-v2/phase0/rc3/{}/files.json",
                        phase0_archive_directory(row.package)
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
                    let slice_path =
                        audited_slice_path(row.package, archive_path, start_byte, end_byte);
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
                    // `*/ ` is expected there and nowhere else.
                    let text = String::from_utf8(slice.clone()).unwrap();
                    let body = text.strip_prefix(" */ ").unwrap_or(&text);
                    assert!(
                        body.starts_with(&format!("function {}(", row.export))
                            || body.starts_with(&format!("const {} = ", row.export)),
                        "{slice_path} does not begin with {}'s definition: {:?}",
                        row.export,
                        &body[..body.len().min(60)]
                    );

                    // 4. And, when the archive is on disk, that those bytes are
                    //    still the archive's bytes at the cited offsets — the
                    //    one check the checked-in slice cannot make. Skipping
                    //    is allowed, silently skipping under a verification run
                    //    is not.
                    match std::env::var("SOLID_CHECKER_RC3_ARCHIVE_ROOT") {
                        Ok(archive_root) if !archive_root.is_empty() => {
                            let file = std::path::Path::new(&archive_root)
                                .join(row.package)
                                .join(archive_path);
                            let bytes = std::fs::read(&file).unwrap_or_else(|error| {
                                panic!(
                                    "SOLID_CHECKER_RC3_ARCHIVE_ROOT is set, but {}: {error}",
                                    file.display()
                                )
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
                        _ => assert_ne!(
                            std::env::var("SOLID_CHECKER_EXPECT_PROBE_PINS").as_deref(),
                            Ok("1"),
                            "SOLID_CHECKER_EXPECT_PROBE_PINS=1, but \
                             SOLID_CHECKER_RC3_ARCHIVE_ROOT is unset: the cited \
                             ranges would only be checked against the \
                             checked-in slices, never against the archive they \
                             claim to quote. scripts/verify.sh exports it from \
                             the tsc-oracle install."
                        ),
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
        // 23 summary citations alone. Five rows, three bundles each.
        assert_eq!(
            implementation_citations, 15,
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
        // (package, export, domain) -> whether every case closes it
        let mut observed = BTreeMap::<(String, String, CallClaimDomain), bool>::new();
        for (_, bytes) in &documents {
            let document: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            let package = document["package"]["name"].as_str().unwrap().to_owned();
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
                                .entry((package.clone(), export.clone(), domain))
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
            BTreeSet::from([CallClaimDomain::Creates]),
            "a new domain was added to the table without widening this comparison"
        );

        let from_json: BTreeSet<(String, String, CallClaimDomain)> = observed
            .into_iter()
            .filter(|((_, _, domain), closed)| *closed && admitted.contains(domain))
            .map(|(key, _)| key)
            .collect();

        // The second source. Every entry names an audit document and a section
        // heading, and both must exist — a hand census that cannot be found is
        // not authority, it is an assertion.
        let root = repository_root();
        let mut implementation_closed = BTreeSet::new();
        let mut implementation_withheld = BTreeSet::new();
        for (package, export, domain, audit, section, verdict) in IMPLEMENTATION_AUDITED {
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
                    .any(|archive| archive.name == *package),
                "{package}:{export} names an archive with no audited tuple"
            );
            let text = std::fs::read_to_string(root.join(audit)).unwrap_or_else(|error| {
                panic!("{package}:{export} names audit {audit}, which cannot be read: {error}")
            });
            assert!(
                text.lines().any(|line| line == *section),
                "{package}:{export} names section {section:?}, which {audit} does not contain \
                 verbatim"
            );
            let key = ((*package).to_owned(), (*export).to_owned(), *domain);
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
        let shipped: BTreeSet<(String, String, CallClaimDomain)> = NEGATIVE_ROWS
            .iter()
            .map(|row| (row.package.to_owned(), row.export.to_owned(), row.domain))
            .collect();
        let withheld: BTreeSet<(String, String, CallClaimDomain)> = WITHHELD
            .iter()
            .map(|(package, export, domain)| ((*package).to_owned(), (*export).to_owned(), *domain))
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

        // 23 from the audited contract documents (25 closures, minus `hydrate`
        // and `createEffect`, both withheld) plus the 5 the hand implementation
        // census closed.
        assert_eq!(from_json.len(), 25);
        assert_eq!(implementation_closed.len(), 5);
        assert_eq!(shipped.len(), 28);

        // The two authorities must not be confusable from the row alone: a row
        // the hand census closed cites runtime bytes, and every other row cites
        // a summary. Otherwise an Implementation citation could ship with no
        // entry above, escaping every check in the loop.
        for row in NEGATIVE_ROWS {
            let key = (row.package.to_owned(), row.export.to_owned(), row.domain);
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
                implementation_closed.contains(&key),
                "{key:?}'s citation kind disagrees with IMPLEMENTATION_AUDITED"
            );
            assert!(
                cites_implementation != cites_summary,
                "{key:?} mixes citation kinds; a row has one authority"
            );
        }
    }

    /// `render` publishes a `create`, so it has no row — and neither does any
    /// export the audit leaves open. The withheld `hydrate` row is pinned in
    /// the same place, because a future re-audit that corrects `hydrate` has to
    /// come through here.
    #[test]
    fn exports_that_publish_the_operation_or_are_withheld_stay_silent() {
        assert!(!Solid2.negative_claim_authority().denies(
            "@solidjs/web",
            "render",
            CallClaimDomain::Creates
        ));
        assert!(!Solid2.negative_claim_authority().denies(
            "@solidjs/web",
            "hydrate",
            CallClaimDomain::Creates
        ));
        assert!(Solid2.negative_claim_authority().denies(
            "@solidjs/web",
            "clientOnly",
            CallClaimDomain::Creates
        ));

        // The two `solid-js` server-condition withholdings, pinned by answer
        // and not only by list membership. `createEffect` is the row this
        // dialect *withdrew* on 2026-09-04: its audited document closes
        // `creates: []`, and the answer here must still be silence.
        let authority = Solid2.negative_claim_authority();
        assert!(!authority.denies("solid-js", "createEffect", CallClaimDomain::Creates));
        assert!(!authority.denies("solid-js", "createSignal", CallClaimDomain::Creates));

        // And the five the hand implementation census closed, which are keyed
        // to `@solidjs/signals` — the archive the declaration resolves into —
        // not to `solid-js`.
        for export in [
            "createRoot",
            "createSignal",
            "getOwner",
            "onCleanup",
            "untrack",
        ] {
            assert!(
                authority.denies("@solidjs/signals", export, CallClaimDomain::Creates),
                "@solidjs/signals:{export} lost its implementation-audited row"
            );
            assert!(
                !authority.denies("solid-js", export, CallClaimDomain::Creates),
                "solid-js:{export} answered from @solidjs/signals' row; the row is archive-keyed"
            );
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

    /// An export outside the audited document, and one the vocabulary does not
    /// spell canonically, both answer nothing.
    #[test]
    fn unaudited_and_non_canonical_exports_answer_nothing() {
        let authority = Solid2.negative_claim_authority();
        // Audited archive, real 2.0 export, no summary in the document and no
        // hand census either.
        assert!(!authority.denies(
            "@solidjs/signals",
            "createContext",
            CallClaimDomain::Creates
        ));
        assert!(!authority.denies("solid-js", "createRenderEffect", CallClaimDomain::Creates));
        // Audited archive, closed in the document, not a canonical primitive.
        assert!(!authority.denies("@solidjs/signals", "isEqual", CallClaimDomain::Creates));
        assert!(!authority.denies("@solidjs/web", "applyRef", CallClaimDomain::Creates));
        // Not an audited archive at all.
        assert!(!authority.denies("@solidjs/router", "createMemo", CallClaimDomain::Creates));
        assert!(authority.archives_named("@solidjs/router").next().is_none());
    }

    /// Every archive tuple is the audited document's own `package` block, all
    /// four fields.
    #[test]
    fn audited_archive_tuples_match_the_documents_package_block() {
        let documents = audited_documents();
        for archive in AUDITED_ARCHIVES {
            let mut seen = 0usize;
            for (path, bytes) in &documents {
                let document: serde_json::Value = serde_json::from_slice(bytes).unwrap();
                if document["package"]["name"].as_str() != Some(archive.name) {
                    continue;
                }
                seen += 1;
                assert_eq!(
                    document["package"]["version"].as_str(),
                    Some(archive.version),
                    "{path} disagrees about {}'s version",
                    archive.name
                );
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
            assert!(seen > 0, "{} has no audited document", archive.name);
        }

        // Every audited package in the directory is listed. An archive read
        // but unlisted would make "never looked" and "found nothing" the same
        // answer, which is the distinction the list exists for.
        for (path, bytes) in &documents {
            let document: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            let name = document["package"]["name"].as_str().unwrap();
            assert!(
                AUDITED_ARCHIVES.iter().any(|archive| archive.name == name),
                "{path} audits {name}, which the archive list omits"
            );
        }
    }

    /// Rows are sorted, unique, keyed to a listed archive, and spelled
    /// canonically.
    #[test]
    fn negative_rows_are_sorted_unique_and_canonical() {
        let mut previous: Option<(&str, &str, CallClaimDomain)> = None;
        for row in NEGATIVE_ROWS {
            let key = (row.package, row.export, row.domain);
            if let Some(previous) = previous {
                assert!(previous < key, "{key:?} is out of order after {previous:?}");
            }
            previous = Some(key);
            assert!(
                AUDITED_ARCHIVES
                    .iter()
                    .any(|archive| archive.name == row.package),
                "{key:?} names an archive with no audited tuple"
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
