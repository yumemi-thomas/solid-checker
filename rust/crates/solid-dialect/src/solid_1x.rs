//! Solid 1.x.
//!
//! Every name here comes from `docs/solid-1x-api-surface.md`, which was
//! extracted from the published `solid-js@1.9.14` package rather than from
//! documentation or memory. That file states the rule this module follows:
//! **do not add a name to the vocabulary that is not on that list.** If
//! something looks missing, re-extract from the package first.
//!
//! The vocabulary is deliberately narrower than that list, and narrower than
//! the bundled 1.x contract's 144 exports: a name earns a place here when the
//! checker models a reactive obligation for it. `createUniqueId` is a real 1.x
//! export with nothing to say about reactivity, and it is not here.
//!
//! The dialect's negative authority ([`NEGATIVE_AUTHORITY`]) audits exactly one
//! archive, `solid-js@1.9.14`, and denies `creates` for sixteen primitives read
//! by hand out of its six published bundles
//! (`docs/package-contract-v2/audits/2026-09-12-solid-1x-1.9.14-core-primitives-creates.md`).
//! It cites no bundled solid-v1 JSON document: those documents' `creates: []`
//! closures were written by the 2026-08-28 migration over a domain the
//! original audit never examined, and are not authority.

use crate::{
    AuditedArchive, AuditedCitation, Boundary, CallClaimDomain, CallbackOwner, CleanupRule,
    Dialect, DialectNegativeAuthority, Execution, NegativeClaimRow, Primitive, ReactiveRole,
    ResultSlot, TrackedCallbackTiming, Version, lookup, reverse,
};

/// Solid 1.x.
#[derive(Clone, Copy, Debug, Default)]
pub struct Solid1x;

/// The exact published archive the Solid 1.x negative rows were read against.
///
/// All four fields are the identity (ADR 0005 objection 1, ADR 0007). The
/// tuple is the `package` block of every `pkg/contracts/bundled/solid-v1/*.json`
/// document, re-established from the npm registry by
/// `scripts/audit-solid-1x.mjs` (`benchmarks/package-contract-v2/phase0/solid-1x/`)
/// and matching the tsc-oracle `v1` install byte for byte on every cited file.
const AUDITED_ARCHIVES: &[AuditedArchive] = &[AuditedArchive {
    name: "solid-js",
    version: "1.9.14",
    integrity: "sha512-sAEXC0Kk0S1EDg+8ysEWJDbYhA3RRoEjwuySUGlKIemeo0I5YZfOyumNjNs9Sv3y2nmhD+0rW66ag2HsMuQiGQ==",
    manifest_sha256: "52ee61ea826e59a5ec36f6ce91bafb7d38bf99ab04b3a6217993fb17af6037c0",
}];

/// The hand implementation census over the exact published 1.9.14 bytes of
/// eleven core primitives (2026-09-12) plus five more (2026-09-13, §§ 13–17),
/// and the source of every row below.
const SOLID1_CORE_PRIMITIVES_AUDIT: &str =
    "docs/package-contract-v2/audits/2026-09-12-solid-1x-1.9.14-core-primitives-creates.md";

/// What the 1.x audit **denies**: sixteen `creates` rows for `solid-js@1.9.14`,
/// every one read out of the archive's own runtime bytes by hand
/// ([`SOLID1_CORE_PRIMITIVES_AUDIT`]) and cited with
/// [`AuditedCitation::Implementation`] into all six `exports["."]` bundles.
///
/// # Why there is no `Summary` row here
///
/// The nineteen `pkg/contracts/bundled/solid-v1/*.json` documents *do* close
/// `creates: []` for all 129 exports, and it would be one mechanical
/// extraction to turn that into rows. That closure is not an audit. The
/// hand-audited `solid-js@1.9.14` contract before commit `474c101f` ("migrate
/// package contracts to normalized v2", 2026-08-28) was schema 1, which had
/// **no claim domains at all**; the `creates: []` **closed** in today's
/// documents was introduced *by the migration*, over a domain the audit never
/// examined — the negative claim manufactured from missing knowledge that
/// ADR 0005 names as inadmissible. `docs/precision-backlog.md` records the
/// finding. So no row cites those documents, and the derivation test below
/// asserts that the JSON-derivable set contributes **nothing** to this table.
///
/// # `creates`, and only `creates`
///
/// The audit reads one domain. `reads`, `callbacks` and `returns` stay
/// withheld for every 1.x primitive until each has its own audit; the
/// admitted-domain assertion in the tests is `{Creates}`, and widening it is
/// a decision.
///
/// # The reading the rows rest on
///
/// `Scheduler` and `ExternalSourceConfig` are `null` until a caller installs
/// them through the separate exports `enableScheduling` and
/// `enableExternalSource`; every reach from these bodies into the scheduler's
/// lazily built `MessageChannel` or into an external-source factory is through
/// module state the caller installed — an installed hook (audit § 0 H1), and
/// in any case a module-level binding, which `semantic-model.md` § creates
/// excludes by name. If that reading is ever reversed, the seven rows that
/// reach the update cycle (`createSignal`, `createMemo`, `createComputed`,
/// `createRenderEffect`, `createEffect`, `batch`, `onMount`) must be withheld
/// together.
const NEGATIVE_ROWS: &[NegativeClaimRow] = &[
    // R9 of SOLID1_CORE_PRIMITIVES_AUDIT § 0. Six bundles, one per
    // export condition; the `.js`/`.cjs` twins are byte-identical, which the
    // equal `slice_sha256` values state rather than imply.
    NegativeClaimRow {
        package: "solid-js",
        export: "batch",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 9. `batch` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.js",
                file_sha256: "b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d",
                start_byte: 13379,
                end_byte: 13433,
                slice_sha256: "094122e29711304fd0249a882f0b169fd7f02462bc67127f2a29c9f352429c6d",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 9. `batch` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.cjs",
                file_sha256: "9ac2a6a5a0d43566c3dbda2fe440b914d37e6d7d5cdd5ebaa7f798c90d26fc29",
                start_byte: 13394,
                end_byte: 13448,
                slice_sha256: "094122e29711304fd0249a882f0b169fd7f02462bc67127f2a29c9f352429c6d",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 9. `batch` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.js",
                file_sha256: "4fadb6529e3a657f0d31f0237e976aeb6f859b0d5bccd58ea98bc9eb4413f010",
                start_byte: 13977,
                end_byte: 14031,
                slice_sha256: "094122e29711304fd0249a882f0b169fd7f02462bc67127f2a29c9f352429c6d",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 9. `batch` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.cjs",
                file_sha256: "3b62ac9d987c6d447b90b03891186ed374c4139d0f93a323a6bdc8b42c3d0e72",
                start_byte: 13992,
                end_byte: 14046,
                slice_sha256: "094122e29711304fd0249a882f0b169fd7f02462bc67127f2a29c9f352429c6d",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 9. `batch` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.js",
                file_sha256: "dfa0ec736228fb544ed655c4008f8ac987543118100bfc96eaec828db07331f0",
                start_byte: 2266,
                end_byte: 2303,
                slice_sha256: "86503a4b75ce02b039de8b8d06606caeba95efbcd646849180ec2d3f6b616ecd",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 9. `batch` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.cjs",
                file_sha256: "093e0bdc3b616e281b601f8ef82e3d14c6472d8b4540606d1f9dc4800c13063a",
                start_byte: 2281,
                end_byte: 2318,
                slice_sha256: "86503a4b75ce02b039de8b8d06606caeba95efbcd646849180ec2d3f6b616ecd",
            },
        ],
    },
    // R6 of SOLID1_CORE_PRIMITIVES_AUDIT § 0. Six bundles, one per
    // export condition; the `.js`/`.cjs` twins are byte-identical, which the
    // equal `slice_sha256` values state rather than imply.
    NegativeClaimRow {
        package: "solid-js",
        export: "createComputed",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 7. `createComputed` and `createRenderEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.js",
                file_sha256: "b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d",
                start_byte: 5753,
                end_byte: 5951,
                slice_sha256: "6924747b55d9a6f0e184548d75ab167d23c07b96e46bfe9a985bd290a0ae8699",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 7. `createComputed` and `createRenderEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.cjs",
                file_sha256: "9ac2a6a5a0d43566c3dbda2fe440b914d37e6d7d5cdd5ebaa7f798c90d26fc29",
                start_byte: 5768,
                end_byte: 5966,
                slice_sha256: "6924747b55d9a6f0e184548d75ab167d23c07b96e46bfe9a985bd290a0ae8699",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 7. `createComputed` and `createRenderEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.js",
                file_sha256: "4fadb6529e3a657f0d31f0237e976aeb6f859b0d5bccd58ea98bc9eb4413f010",
                start_byte: 6291,
                end_byte: 6499,
                slice_sha256: "a5cec8d63977e87f10967d5d5997cb7bd0eda17bd54953c80e1cf699eefa821a",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 7. `createComputed` and `createRenderEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.cjs",
                file_sha256: "3b62ac9d987c6d447b90b03891186ed374c4139d0f93a323a6bdc8b42c3d0e72",
                start_byte: 6306,
                end_byte: 6514,
                slice_sha256: "a5cec8d63977e87f10967d5d5997cb7bd0eda17bd54953c80e1cf699eefa821a",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 7. `createComputed` and `createRenderEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.js",
                file_sha256: "dfa0ec736228fb544ed655c4008f8ac987543118100bfc96eaec828db07331f0",
                start_byte: 1622,
                end_byte: 1791,
                slice_sha256: "81fee305f25c3be61304ff283418e0f94866bb676e3b1fab6d328dd2d2a7848d",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 7. `createComputed` and `createRenderEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.cjs",
                file_sha256: "093e0bdc3b616e281b601f8ef82e3d14c6472d8b4540606d1f9dc4800c13063a",
                start_byte: 1637,
                end_byte: 1806,
                slice_sha256: "81fee305f25c3be61304ff283418e0f94866bb676e3b1fab6d328dd2d2a7848d",
            },
        ],
    },
    // R12 of SOLID1_CORE_PRIMITIVES_AUDIT § 0 (2026-09-13, § 13). Six bundles,
    // one per export condition; the `.js`/`.cjs` twins are byte-identical.
    NegativeClaimRow {
        package: "solid-js",
        export: "createContext",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 13. `createContext` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.js",
                file_sha256: "b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d",
                start_byte: 16136,
                end_byte: 16292,
                slice_sha256: "1696acc67a62350f304f56cb4fa0ec7b7f4636a7b2b250fb2ea126152268983f",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 13. `createContext` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.cjs",
                file_sha256: "9ac2a6a5a0d43566c3dbda2fe440b914d37e6d7d5cdd5ebaa7f798c90d26fc29",
                start_byte: 16151,
                end_byte: 16307,
                slice_sha256: "1696acc67a62350f304f56cb4fa0ec7b7f4636a7b2b250fb2ea126152268983f",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 13. `createContext` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.js",
                file_sha256: "4fadb6529e3a657f0d31f0237e976aeb6f859b0d5bccd58ea98bc9eb4413f010",
                start_byte: 17441,
                end_byte: 17606,
                slice_sha256: "d5094c97565d8ced495c6841585a8c5392f30a031ba541eb3469df9a63b58392",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 13. `createContext` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.cjs",
                file_sha256: "3b62ac9d987c6d447b90b03891186ed374c4139d0f93a323a6bdc8b42c3d0e72",
                start_byte: 17456,
                end_byte: 17621,
                slice_sha256: "d5094c97565d8ced495c6841585a8c5392f30a031ba541eb3469df9a63b58392",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 13. `createContext` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.js",
                file_sha256: "dfa0ec736228fb544ed655c4008f8ac987543118100bfc96eaec828db07331f0",
                start_byte: 3399,
                end_byte: 3546,
                slice_sha256: "80db7be4958920897b5e16afd680516c288651ff5435db12e24889842ac6dfd5",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 13. `createContext` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.cjs",
                file_sha256: "093e0bdc3b616e281b601f8ef82e3d14c6472d8b4540606d1f9dc4800c13063a",
                start_byte: 3414,
                end_byte: 3561,
                slice_sha256: "80db7be4958920897b5e16afd680516c288651ff5435db12e24889842ac6dfd5",
            },
        ],
    },
    // R8 of SOLID1_CORE_PRIMITIVES_AUDIT § 0. Six bundles, one per
    // export condition; the `.js`/`.cjs` twins are byte-identical, which the
    // equal `slice_sha256` values state rather than imply.
    NegativeClaimRow {
        package: "solid-js",
        export: "createEffect",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 8. `createEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.js",
                file_sha256: "b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d",
                start_byte: 6156,
                end_byte: 6471,
                slice_sha256: "2ee7f40eb4c1072a21c2b8d3cc9834160f4fbdb0f6d33fe199fd1b4563b92759",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 8. `createEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.cjs",
                file_sha256: "9ac2a6a5a0d43566c3dbda2fe440b914d37e6d7d5cdd5ebaa7f798c90d26fc29",
                start_byte: 6171,
                end_byte: 6486,
                slice_sha256: "2ee7f40eb4c1072a21c2b8d3cc9834160f4fbdb0f6d33fe199fd1b4563b92759",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 8. `createEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.js",
                file_sha256: "4fadb6529e3a657f0d31f0237e976aeb6f859b0d5bccd58ea98bc9eb4413f010",
                start_byte: 6714,
                end_byte: 7039,
                slice_sha256: "32b128182dbff28a75297decd8e867e74c4a1c4d60f385f2c618ebb8b15d7a2c",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 8. `createEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.cjs",
                file_sha256: "3b62ac9d987c6d447b90b03891186ed374c4139d0f93a323a6bdc8b42c3d0e72",
                start_byte: 6729,
                end_byte: 7054,
                slice_sha256: "32b128182dbff28a75297decd8e867e74c4a1c4d60f385f2c618ebb8b15d7a2c",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 8. `createEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.js",
                file_sha256: "dfa0ec736228fb544ed655c4008f8ac987543118100bfc96eaec828db07331f0",
                start_byte: 1835,
                end_byte: 1870,
                slice_sha256: "fae26341d86979aaa089e11a41fef19468e041d54300a44b308c03d01e451aa9",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 8. `createEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.cjs",
                file_sha256: "093e0bdc3b616e281b601f8ef82e3d14c6472d8b4540606d1f9dc4800c13063a",
                start_byte: 1850,
                end_byte: 1885,
                slice_sha256: "fae26341d86979aaa089e11a41fef19468e041d54300a44b308c03d01e451aa9",
            },
        ],
    },
    // R5 of SOLID1_CORE_PRIMITIVES_AUDIT § 0. Six bundles, one per
    // export condition; the `.js`/`.cjs` twins are byte-identical, which the
    // equal `slice_sha256` values state rather than imply.
    NegativeClaimRow {
        package: "solid-js",
        export: "createMemo",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 6. `createMemo` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.js",
                file_sha256: "b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d",
                start_byte: 6834,
                end_byte: 7261,
                slice_sha256: "ac6dd73e0270fd20e9b46a2700bdd6bbd2ddba74e7df39b920960213ce7e551a",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 6. `createMemo` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.cjs",
                file_sha256: "9ac2a6a5a0d43566c3dbda2fe440b914d37e6d7d5cdd5ebaa7f798c90d26fc29",
                start_byte: 6849,
                end_byte: 7276,
                slice_sha256: "ac6dd73e0270fd20e9b46a2700bdd6bbd2ddba74e7df39b920960213ce7e551a",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 6. `createMemo` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.js",
                file_sha256: "4fadb6529e3a657f0d31f0237e976aeb6f859b0d5bccd58ea98bc9eb4413f010",
                start_byte: 7412,
                end_byte: 7849,
                slice_sha256: "ad27d2ef9fb668fa3bacfa07334042ea78727815bc6741dd50ef034273de2b32",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 6. `createMemo` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.cjs",
                file_sha256: "3b62ac9d987c6d447b90b03891186ed374c4139d0f93a323a6bdc8b42c3d0e72",
                start_byte: 7427,
                end_byte: 7864,
                slice_sha256: "ad27d2ef9fb668fa3bacfa07334042ea78727815bc6741dd50ef034273de2b32",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 6. `createMemo` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.js",
                file_sha256: "dfa0ec736228fb544ed655c4008f8ac987543118100bfc96eaec828db07331f0",
                start_byte: 1935,
                end_byte: 2131,
                slice_sha256: "c5ad2eb704d1ba3992a74d06e14569ca2a7e79350aab62f6a2c6316919516dcf",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 6. `createMemo` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.cjs",
                file_sha256: "093e0bdc3b616e281b601f8ef82e3d14c6472d8b4540606d1f9dc4800c13063a",
                start_byte: 1950,
                end_byte: 2146,
                slice_sha256: "c5ad2eb704d1ba3992a74d06e14569ca2a7e79350aab62f6a2c6316919516dcf",
            },
        ],
    },
    // R7 of SOLID1_CORE_PRIMITIVES_AUDIT § 0. Six bundles, one per
    // export condition; the `.js`/`.cjs` twins are byte-identical, which the
    // equal `slice_sha256` values state rather than imply.
    NegativeClaimRow {
        package: "solid-js",
        export: "createRenderEffect",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 7. `createComputed` and `createRenderEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.js",
                file_sha256: "b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d",
                start_byte: 5952,
                end_byte: 6155,
                slice_sha256: "f7c3e6ea4209f24fa6dfb714be9022b8af972961fbc57a8bc5df33f9c6d9ab97",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 7. `createComputed` and `createRenderEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.cjs",
                file_sha256: "9ac2a6a5a0d43566c3dbda2fe440b914d37e6d7d5cdd5ebaa7f798c90d26fc29",
                start_byte: 5967,
                end_byte: 6170,
                slice_sha256: "f7c3e6ea4209f24fa6dfb714be9022b8af972961fbc57a8bc5df33f9c6d9ab97",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 7. `createComputed` and `createRenderEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.js",
                file_sha256: "4fadb6529e3a657f0d31f0237e976aeb6f859b0d5bccd58ea98bc9eb4413f010",
                start_byte: 6500,
                end_byte: 6713,
                slice_sha256: "04311e342d1c0421afbb43054748f9d7ba7fead9439b34c8f348346477bb98aa",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 7. `createComputed` and `createRenderEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.cjs",
                file_sha256: "3b62ac9d987c6d447b90b03891186ed374c4139d0f93a323a6bdc8b42c3d0e72",
                start_byte: 6515,
                end_byte: 6728,
                slice_sha256: "04311e342d1c0421afbb43054748f9d7ba7fead9439b34c8f348346477bb98aa",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 7. `createComputed` and `createRenderEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.js",
                file_sha256: "dfa0ec736228fb544ed655c4008f8ac987543118100bfc96eaec828db07331f0",
                start_byte: 1792,
                end_byte: 1834,
                slice_sha256: "fbec132e9893ce500cee7699dc1177f7c82de0ea1a6e6af809a48c3ad1a8f887",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 7. `createComputed` and `createRenderEffect` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.cjs",
                file_sha256: "093e0bdc3b616e281b601f8ef82e3d14c6472d8b4540606d1f9dc4800c13063a",
                start_byte: 1807,
                end_byte: 1849,
                slice_sha256: "fbec132e9893ce500cee7699dc1177f7c82de0ea1a6e6af809a48c3ad1a8f887",
            },
        ],
    },
    // R4 of SOLID1_CORE_PRIMITIVES_AUDIT § 0. Six bundles, one per
    // export condition; the `.js`/`.cjs` twins are byte-identical, which the
    // equal `slice_sha256` values state rather than imply.
    NegativeClaimRow {
        package: "solid-js",
        export: "createSignal",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 5. `createSignal` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.js",
                file_sha256: "b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d",
                start_byte: 5233,
                end_byte: 5752,
                slice_sha256: "0ef6fad8b1835199b7f1f016e841090bef3171d325d42a0c2fab0f21a81b761b",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 5. `createSignal` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.cjs",
                file_sha256: "9ac2a6a5a0d43566c3dbda2fe440b914d37e6d7d5cdd5ebaa7f798c90d26fc29",
                start_byte: 5248,
                end_byte: 5767,
                slice_sha256: "0ef6fad8b1835199b7f1f016e841090bef3171d325d42a0c2fab0f21a81b761b",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 5. `createSignal` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.js",
                file_sha256: "4fadb6529e3a657f0d31f0237e976aeb6f859b0d5bccd58ea98bc9eb4413f010",
                start_byte: 5553,
                end_byte: 6290,
                slice_sha256: "e253c8edc9873981823ab5583e79d0ea1b99162d4072b0251a48bdda01bb592e",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 5. `createSignal` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.cjs",
                file_sha256: "3b62ac9d987c6d447b90b03891186ed374c4139d0f93a323a6bdc8b42c3d0e72",
                start_byte: 5568,
                end_byte: 6305,
                slice_sha256: "e253c8edc9873981823ab5583e79d0ea1b99162d4072b0251a48bdda01bb592e",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 5. `createSignal` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.js",
                file_sha256: "dfa0ec736228fb544ed655c4008f8ac987543118100bfc96eaec828db07331f0",
                start_byte: 1485,
                end_byte: 1621,
                slice_sha256: "e345c3338166b4be6ce6355e58814427cf497cd6503812ea2b8b108c50aa8ae5",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 5. `createSignal` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.cjs",
                file_sha256: "093e0bdc3b616e281b601f8ef82e3d14c6472d8b4540606d1f9dc4800c13063a",
                start_byte: 1500,
                end_byte: 1636,
                slice_sha256: "e345c3338166b4be6ce6355e58814427cf497cd6503812ea2b8b108c50aa8ae5",
            },
        ],
    },
    // R13 of SOLID1_CORE_PRIMITIVES_AUDIT § 0 (2026-09-13, § 14). Six bundles,
    // one per export condition; the `.js`/`.cjs` twins are byte-identical.
    NegativeClaimRow {
        package: "solid-js",
        export: "getOwner",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 14. `getOwner` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.js",
                file_sha256: "b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d",
                start_byte: 14843,
                end_byte: 14882,
                slice_sha256: "283aec631382e4285673f2025f18ac178cb984770a65232d4536af4d7c1df846",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 14. `getOwner` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.cjs",
                file_sha256: "9ac2a6a5a0d43566c3dbda2fe440b914d37e6d7d5cdd5ebaa7f798c90d26fc29",
                start_byte: 14858,
                end_byte: 14897,
                slice_sha256: "283aec631382e4285673f2025f18ac178cb984770a65232d4536af4d7c1df846",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 14. `getOwner` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.js",
                file_sha256: "4fadb6529e3a657f0d31f0237e976aeb6f859b0d5bccd58ea98bc9eb4413f010",
                start_byte: 15526,
                end_byte: 15565,
                slice_sha256: "283aec631382e4285673f2025f18ac178cb984770a65232d4536af4d7c1df846",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 14. `getOwner` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.cjs",
                file_sha256: "3b62ac9d987c6d447b90b03891186ed374c4139d0f93a323a6bdc8b42c3d0e72",
                start_byte: 15541,
                end_byte: 15580,
                slice_sha256: "283aec631382e4285673f2025f18ac178cb984770a65232d4536af4d7c1df846",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 14. `getOwner` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.js",
                file_sha256: "dfa0ec736228fb544ed655c4008f8ac987543118100bfc96eaec828db07331f0",
                start_byte: 3707,
                end_byte: 3746,
                slice_sha256: "283aec631382e4285673f2025f18ac178cb984770a65232d4536af4d7c1df846",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 14. `getOwner` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.cjs",
                file_sha256: "093e0bdc3b616e281b601f8ef82e3d14c6472d8b4540606d1f9dc4800c13063a",
                start_byte: 3722,
                end_byte: 3761,
                slice_sha256: "283aec631382e4285673f2025f18ac178cb984770a65232d4536af4d7c1df846",
            },
        ],
    },
    // R14 of SOLID1_CORE_PRIMITIVES_AUDIT § 0 (2026-09-13, § 15). Six bundles,
    // one per export condition; the `.js`/`.cjs` twins are byte-identical.
    NegativeClaimRow {
        package: "solid-js",
        export: "mapArray",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 15. `mapArray` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.js",
                file_sha256: "b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d",
                start_byte: 32751,
                end_byte: 35782,
                slice_sha256: "841e311992bdd52ba1ac8d4bebd5eb777e43c899316822f83a1def6692f93cc5",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 15. `mapArray` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.cjs",
                file_sha256: "9ac2a6a5a0d43566c3dbda2fe440b914d37e6d7d5cdd5ebaa7f798c90d26fc29",
                start_byte: 32766,
                end_byte: 35797,
                slice_sha256: "841e311992bdd52ba1ac8d4bebd5eb777e43c899316822f83a1def6692f93cc5",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 15. `mapArray` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.js",
                file_sha256: "4fadb6529e3a657f0d31f0237e976aeb6f859b0d5bccd58ea98bc9eb4413f010",
                start_byte: 34543,
                end_byte: 37612,
                slice_sha256: "1736b368d62b19086cefbc7dd4525a3eb3332205eff0a98a7094631b3d7b69b0",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 15. `mapArray` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.cjs",
                file_sha256: "3b62ac9d987c6d447b90b03891186ed374c4139d0f93a323a6bdc8b42c3d0e72",
                start_byte: 34558,
                end_byte: 37627,
                slice_sha256: "1736b368d62b19086cefbc7dd4525a3eb3332205eff0a98a7094631b3d7b69b0",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 15. `mapArray` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.js",
                file_sha256: "dfa0ec736228fb544ed655c4008f8ac987543118100bfc96eaec828db07331f0",
                start_byte: 5056,
                end_byte: 5336,
                slice_sha256: "6a3f9b16803cb02ed200a7feb4b9176922e3f8fc1c90ea34989c9283a757b79a",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 15. `mapArray` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.cjs",
                file_sha256: "093e0bdc3b616e281b601f8ef82e3d14c6472d8b4540606d1f9dc4800c13063a",
                start_byte: 5071,
                end_byte: 5351,
                slice_sha256: "6a3f9b16803cb02ed200a7feb4b9176922e3f8fc1c90ea34989c9283a757b79a",
            },
        ],
    },
    // R3 of SOLID1_CORE_PRIMITIVES_AUDIT § 0. Six bundles, one per
    // export condition; the `.js`/`.cjs` twins are byte-identical, which the
    // equal `slice_sha256` values state rather than imply.
    NegativeClaimRow {
        package: "solid-js",
        export: "mergeProps",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 4. `mergeProps` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.js",
                file_sha256: "b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d",
                start_byte: 38566,
                end_byte: 40717,
                slice_sha256: "70419fd8a5e57ae7733371e89e32abad4c8bede4287b8cb022a0ebd54209aeb7",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 4. `mergeProps` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.cjs",
                file_sha256: "9ac2a6a5a0d43566c3dbda2fe440b914d37e6d7d5cdd5ebaa7f798c90d26fc29",
                start_byte: 38581,
                end_byte: 40732,
                slice_sha256: "70419fd8a5e57ae7733371e89e32abad4c8bede4287b8cb022a0ebd54209aeb7",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 4. `mergeProps` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.js",
                file_sha256: "4fadb6529e3a657f0d31f0237e976aeb6f859b0d5bccd58ea98bc9eb4413f010",
                start_byte: 40429,
                end_byte: 42580,
                slice_sha256: "70419fd8a5e57ae7733371e89e32abad4c8bede4287b8cb022a0ebd54209aeb7",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 4. `mergeProps` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.cjs",
                file_sha256: "3b62ac9d987c6d447b90b03891186ed374c4139d0f93a323a6bdc8b42c3d0e72",
                start_byte: 40444,
                end_byte: 42595,
                slice_sha256: "70419fd8a5e57ae7733371e89e32abad4c8bede4287b8cb022a0ebd54209aeb7",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 4. `mergeProps` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.js",
                file_sha256: "dfa0ec736228fb544ed655c4008f8ac987543118100bfc96eaec828db07331f0",
                start_byte: 10453,
                end_byte: 11294,
                slice_sha256: "89c2c1e06f3648b7aa05dc840de9d467f199f56200a754d90e81aa8f5d6c1e3b",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 4. `mergeProps` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.cjs",
                file_sha256: "093e0bdc3b616e281b601f8ef82e3d14c6472d8b4540606d1f9dc4800c13063a",
                start_byte: 10468,
                end_byte: 11309,
                slice_sha256: "89c2c1e06f3648b7aa05dc840de9d467f199f56200a754d90e81aa8f5d6c1e3b",
            },
        ],
    },
    // R10 of SOLID1_CORE_PRIMITIVES_AUDIT § 0. Six bundles, one per
    // export condition; the `.js`/`.cjs` twins are byte-identical, which the
    // equal `slice_sha256` values state rather than imply.
    NegativeClaimRow {
        package: "solid-js",
        export: "on",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 10. `on` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.js",
                file_sha256: "b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d",
                start_byte: 13709,
                end_byte: 14206,
                slice_sha256: "1b3f2ae2dda48a0c1ffa099d75f2770c23eaeda83ca91113f56119dfe56bf11f",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 10. `on` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.cjs",
                file_sha256: "9ac2a6a5a0d43566c3dbda2fe440b914d37e6d7d5cdd5ebaa7f798c90d26fc29",
                start_byte: 13724,
                end_byte: 14221,
                slice_sha256: "1b3f2ae2dda48a0c1ffa099d75f2770c23eaeda83ca91113f56119dfe56bf11f",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 10. `on` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.js",
                file_sha256: "4fadb6529e3a657f0d31f0237e976aeb6f859b0d5bccd58ea98bc9eb4413f010",
                start_byte: 14307,
                end_byte: 14804,
                slice_sha256: "1b3f2ae2dda48a0c1ffa099d75f2770c23eaeda83ca91113f56119dfe56bf11f",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 10. `on` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.cjs",
                file_sha256: "3b62ac9d987c6d447b90b03891186ed374c4139d0f93a323a6bdc8b42c3d0e72",
                start_byte: 14322,
                end_byte: 14819,
                slice_sha256: "1b3f2ae2dda48a0c1ffa099d75f2770c23eaeda83ca91113f56119dfe56bf11f",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 10. `on` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.js",
                file_sha256: "dfa0ec736228fb544ed655c4008f8ac987543118100bfc96eaec828db07331f0",
                start_byte: 2327,
                end_byte: 2659,
                slice_sha256: "3254e3b718d93416e11a60a2ddf9be507e4b6b0edf2c0db24a6f2ae9d4ce0a93",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 10. `on` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.cjs",
                file_sha256: "093e0bdc3b616e281b601f8ef82e3d14c6472d8b4540606d1f9dc4800c13063a",
                start_byte: 2342,
                end_byte: 2674,
                slice_sha256: "3254e3b718d93416e11a60a2ddf9be507e4b6b0edf2c0db24a6f2ae9d4ce0a93",
            },
        ],
    },
    // R15 of SOLID1_CORE_PRIMITIVES_AUDIT § 0 (2026-09-13, § 16). Six bundles,
    // one per export condition; the `.js`/`.cjs` twins are byte-identical.
    NegativeClaimRow {
        package: "solid-js",
        export: "onCleanup",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 16. `onCleanup` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.js",
                file_sha256: "b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d",
                start_byte: 14267,
                end_byte: 14415,
                slice_sha256: "0892207417709049d794f4fcd5517df320cc035e265e30a76fc4730c828ad6fa",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 16. `onCleanup` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.cjs",
                file_sha256: "9ac2a6a5a0d43566c3dbda2fe440b914d37e6d7d5cdd5ebaa7f798c90d26fc29",
                start_byte: 14282,
                end_byte: 14430,
                slice_sha256: "0892207417709049d794f4fcd5517df320cc035e265e30a76fc4730c828ad6fa",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 16. `onCleanup` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.js",
                file_sha256: "4fadb6529e3a657f0d31f0237e976aeb6f859b0d5bccd58ea98bc9eb4413f010",
                start_byte: 14865,
                end_byte: 15098,
                slice_sha256: "7915d5cda23e9d22c27d67cea4fd56f696d54a84fd44f29db389fdf5d068c490",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 16. `onCleanup` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.cjs",
                file_sha256: "3b62ac9d987c6d447b90b03891186ed374c4139d0f93a323a6bdc8b42c3d0e72",
                start_byte: 14880,
                end_byte: 15113,
                slice_sha256: "7915d5cda23e9d22c27d67cea4fd56f696d54a84fd44f29db389fdf5d068c490",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 16. `onCleanup` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.js",
                file_sha256: "dfa0ec736228fb544ed655c4008f8ac987543118100bfc96eaec828db07331f0",
                start_byte: 2684,
                end_byte: 2819,
                slice_sha256: "a9360fbfb9506dd42a0e97c4807f638c29e53d81c3f34afc421274018f6cc71e",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 16. `onCleanup` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.cjs",
                file_sha256: "093e0bdc3b616e281b601f8ef82e3d14c6472d8b4540606d1f9dc4800c13063a",
                start_byte: 2699,
                end_byte: 2834,
                slice_sha256: "a9360fbfb9506dd42a0e97c4807f638c29e53d81c3f34afc421274018f6cc71e",
            },
        ],
    },
    // R11 of SOLID1_CORE_PRIMITIVES_AUDIT § 0. Six bundles, one per
    // export condition; the `.js`/`.cjs` twins are byte-identical, which the
    // equal `slice_sha256` values state rather than imply.
    NegativeClaimRow {
        package: "solid-js",
        export: "onMount",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 11. `onMount` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.js",
                file_sha256: "b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d",
                start_byte: 14207,
                end_byte: 14266,
                slice_sha256: "d911737441911fbb4270c23d8e6212c8f74e9b48d43bb482371e4c8b86d92c03",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 11. `onMount` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.cjs",
                file_sha256: "9ac2a6a5a0d43566c3dbda2fe440b914d37e6d7d5cdd5ebaa7f798c90d26fc29",
                start_byte: 14222,
                end_byte: 14281,
                slice_sha256: "d911737441911fbb4270c23d8e6212c8f74e9b48d43bb482371e4c8b86d92c03",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 11. `onMount` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.js",
                file_sha256: "4fadb6529e3a657f0d31f0237e976aeb6f859b0d5bccd58ea98bc9eb4413f010",
                start_byte: 14805,
                end_byte: 14864,
                slice_sha256: "d911737441911fbb4270c23d8e6212c8f74e9b48d43bb482371e4c8b86d92c03",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 11. `onMount` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.cjs",
                file_sha256: "3b62ac9d987c6d447b90b03891186ed374c4139d0f93a323a6bdc8b42c3d0e72",
                start_byte: 14820,
                end_byte: 14879,
                slice_sha256: "d911737441911fbb4270c23d8e6212c8f74e9b48d43bb482371e4c8b86d92c03",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 11. `onMount` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.js",
                file_sha256: "dfa0ec736228fb544ed655c4008f8ac987543118100bfc96eaec828db07331f0",
                start_byte: 2660,
                end_byte: 2683,
                slice_sha256: "9986562f919977540d15b1c1b73c7ee88a98fa319ac07c4c734050acc8b5863b",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 11. `onMount` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.cjs",
                file_sha256: "093e0bdc3b616e281b601f8ef82e3d14c6472d8b4540606d1f9dc4800c13063a",
                start_byte: 2675,
                end_byte: 2698,
                slice_sha256: "9986562f919977540d15b1c1b73c7ee88a98fa319ac07c4c734050acc8b5863b",
            },
        ],
    },
    // R2 of SOLID1_CORE_PRIMITIVES_AUDIT § 0. Six bundles, one per
    // export condition; the `.js`/`.cjs` twins are byte-identical, which the
    // equal `slice_sha256` values state rather than imply.
    NegativeClaimRow {
        package: "solid-js",
        export: "splitProps",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 3. `splitProps` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.js",
                file_sha256: "b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d",
                start_byte: 40718,
                end_byte: 42233,
                slice_sha256: "0401d0b3a5b32d418ce3e1bb0dd037ad9c56e4da756afb09864f6f6cb071a3b0",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 3. `splitProps` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.cjs",
                file_sha256: "9ac2a6a5a0d43566c3dbda2fe440b914d37e6d7d5cdd5ebaa7f798c90d26fc29",
                start_byte: 40733,
                end_byte: 42248,
                slice_sha256: "0401d0b3a5b32d418ce3e1bb0dd037ad9c56e4da756afb09864f6f6cb071a3b0",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 3. `splitProps` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.js",
                file_sha256: "4fadb6529e3a657f0d31f0237e976aeb6f859b0d5bccd58ea98bc9eb4413f010",
                start_byte: 42581,
                end_byte: 44096,
                slice_sha256: "0401d0b3a5b32d418ce3e1bb0dd037ad9c56e4da756afb09864f6f6cb071a3b0",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 3. `splitProps` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.cjs",
                file_sha256: "3b62ac9d987c6d447b90b03891186ed374c4139d0f93a323a6bdc8b42c3d0e72",
                start_byte: 42596,
                end_byte: 44111,
                slice_sha256: "0401d0b3a5b32d418ce3e1bb0dd037ad9c56e4da756afb09864f6f6cb071a3b0",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 3. `splitProps` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.js",
                file_sha256: "dfa0ec736228fb544ed655c4008f8ac987543118100bfc96eaec828db07331f0",
                start_byte: 11295,
                end_byte: 11750,
                slice_sha256: "213b67a6a309d8c1c32596615f7accffcd6f7208845edb9b9879a28d2f6ec679",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 3. `splitProps` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.cjs",
                file_sha256: "093e0bdc3b616e281b601f8ef82e3d14c6472d8b4540606d1f9dc4800c13063a",
                start_byte: 11310,
                end_byte: 11765,
                slice_sha256: "213b67a6a309d8c1c32596615f7accffcd6f7208845edb9b9879a28d2f6ec679",
            },
        ],
    },
    // R16 of SOLID1_CORE_PRIMITIVES_AUDIT § 0 (2026-09-13, § 17). Six bundles,
    // one per export condition; the `.js`/`.cjs` twins are byte-identical.
    NegativeClaimRow {
        package: "solid-js",
        export: "untrack",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 17. `untrack` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.js",
                file_sha256: "b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d",
                start_byte: 13434,
                end_byte: 13708,
                slice_sha256: "26664788da0606e73eb1b132889ba9bbc52309513e0b9d15ac5ed74dc2cd0a28",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 17. `untrack` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.cjs",
                file_sha256: "9ac2a6a5a0d43566c3dbda2fe440b914d37e6d7d5cdd5ebaa7f798c90d26fc29",
                start_byte: 13449,
                end_byte: 13723,
                slice_sha256: "26664788da0606e73eb1b132889ba9bbc52309513e0b9d15ac5ed74dc2cd0a28",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 17. `untrack` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.js",
                file_sha256: "4fadb6529e3a657f0d31f0237e976aeb6f859b0d5bccd58ea98bc9eb4413f010",
                start_byte: 14032,
                end_byte: 14306,
                slice_sha256: "26664788da0606e73eb1b132889ba9bbc52309513e0b9d15ac5ed74dc2cd0a28",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 17. `untrack` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.cjs",
                file_sha256: "3b62ac9d987c6d447b90b03891186ed374c4139d0f93a323a6bdc8b42c3d0e72",
                start_byte: 14047,
                end_byte: 14321,
                slice_sha256: "26664788da0606e73eb1b132889ba9bbc52309513e0b9d15ac5ed74dc2cd0a28",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 17. `untrack` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.js",
                file_sha256: "dfa0ec736228fb544ed655c4008f8ac987543118100bfc96eaec828db07331f0",
                start_byte: 2304,
                end_byte: 2326,
                slice_sha256: "5c6b46e92c061354239e5dd70806402ff0e676f4bd390dc0a6ac13b39c0b3139",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 17. `untrack` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.cjs",
                file_sha256: "093e0bdc3b616e281b601f8ef82e3d14c6472d8b4540606d1f9dc4800c13063a",
                start_byte: 2319,
                end_byte: 2341,
                slice_sha256: "5c6b46e92c061354239e5dd70806402ff0e676f4bd390dc0a6ac13b39c0b3139",
            },
        ],
    },
    // R1 of SOLID1_CORE_PRIMITIVES_AUDIT § 0. Six bundles, one per
    // export condition; the `.js`/`.cjs` twins are byte-identical, which the
    // equal `slice_sha256` values state rather than imply.
    NegativeClaimRow {
        package: "solid-js",
        export: "useContext",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 2. `useContext` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.js",
                file_sha256: "b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d",
                start_byte: 16293,
                end_byte: 16455,
                slice_sha256: "693f083021eb3de3ae757fdd7d7ce3cf617d913a9a9036b0ca08f8705b734fd9",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 2. `useContext` — archive `solid-js@1.9.14`",
                archive_path: "dist/solid.cjs",
                file_sha256: "9ac2a6a5a0d43566c3dbda2fe440b914d37e6d7d5cdd5ebaa7f798c90d26fc29",
                start_byte: 16308,
                end_byte: 16470,
                slice_sha256: "693f083021eb3de3ae757fdd7d7ce3cf617d913a9a9036b0ca08f8705b734fd9",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 2. `useContext` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.js",
                file_sha256: "4fadb6529e3a657f0d31f0237e976aeb6f859b0d5bccd58ea98bc9eb4413f010",
                start_byte: 17607,
                end_byte: 17769,
                slice_sha256: "693f083021eb3de3ae757fdd7d7ce3cf617d913a9a9036b0ca08f8705b734fd9",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 2. `useContext` — archive `solid-js@1.9.14`",
                archive_path: "dist/dev.cjs",
                file_sha256: "3b62ac9d987c6d447b90b03891186ed374c4139d0f93a323a6bdc8b42c3d0e72",
                start_byte: 17622,
                end_byte: 17784,
                slice_sha256: "693f083021eb3de3ae757fdd7d7ce3cf617d913a9a9036b0ca08f8705b734fd9",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 2. `useContext` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.js",
                file_sha256: "dfa0ec736228fb544ed655c4008f8ac987543118100bfc96eaec828db07331f0",
                start_byte: 3547,
                end_byte: 3706,
                slice_sha256: "80775f0a6f4dc650857a3b80fd8033f64c12a6d7d4832521a82f327e39983cec",
            },
            AuditedCitation::Implementation {
                audit: SOLID1_CORE_PRIMITIVES_AUDIT,
                section: "## 2. `useContext` — archive `solid-js@1.9.14`",
                archive_path: "dist/server.cjs",
                file_sha256: "093e0bdc3b616e281b601f8ef82e3d14c6472d8b4540606d1f9dc4800c13063a",
                start_byte: 3562,
                end_byte: 3721,
                slice_sha256: "80775f0a6f4dc650857a3b80fd8033f64c12a6d7d4832521a82f327e39983cec",
            },
        ],
    },
];

static NEGATIVE_AUTHORITY: DialectNegativeAuthority = DialectNegativeAuthority {
    archives: AUDITED_ARCHIVES,
    rows: NEGATIVE_ROWS,
};

/// Source: `docs/solid-1x-api-surface.md`, sections `solid-js`,
/// `solid-js/store` and the control-flow components. Names the checker does
/// not model — `createUniqueId`, `observable`, the SSR helpers — are omitted
/// deliberately; they carry no reactive obligation.
const TABLE: &[(&str, Primitive)] = &[
    ("batch", Primitive::Batch),
    ("catchError", Primitive::CatchError),
    ("children", Primitive::Children),
    ("createComputed", Primitive::CreateComputed),
    ("createContext", Primitive::CreateContext),
    ("createDeferred", Primitive::CreateDeferred),
    ("createDynamic", Primitive::CreateDynamic),
    ("createEffect", Primitive::CreateEffect),
    ("createMemo", Primitive::CreateMemo),
    ("createMutable", Primitive::CreateMutable),
    ("createReaction", Primitive::CreateReaction),
    ("createRenderEffect", Primitive::CreateRenderEffect),
    ("createResource", Primitive::CreateResource),
    ("createRoot", Primitive::CreateRoot),
    ("createSelector", Primitive::CreateSelector),
    ("createSignal", Primitive::CreateSignal),
    ("createStore", Primitive::CreateStore),
    ("ErrorBoundary", Primitive::ErrorBoundary),
    ("For", Primitive::For),
    ("from", Primitive::From),
    ("getOwner", Primitive::GetOwner),
    ("hydrate", Primitive::Hydrate),
    ("Index", Primitive::Index),
    ("indexArray", Primitive::IndexArray),
    ("lazy", Primitive::Lazy),
    ("mapArray", Primitive::MapArray),
    ("Match", Primitive::Match),
    ("memo", Primitive::WebMemo),
    ("mergeProps", Primitive::MergeProps),
    ("modifyMutable", Primitive::ModifyMutable),
    ("on", Primitive::On),
    ("onCleanup", Primitive::OnCleanup),
    ("onError", Primitive::OnError),
    ("onMount", Primitive::OnMount),
    ("produce", Primitive::Produce),
    ("reconcile", Primitive::Reconcile),
    ("render", Primitive::Render),
    ("runWithOwner", Primitive::RunWithOwner),
    ("Show", Primitive::Show),
    ("splitProps", Primitive::SplitProps),
    ("startTransition", Primitive::StartTransition),
    ("Suspense", Primitive::Suspense),
    ("SuspenseList", Primitive::SuspenseList),
    ("Switch", Primitive::Switch),
    ("untrack", Primitive::Untrack),
    ("unwrap", Primitive::Unwrap),
    ("useContext", Primitive::UseContext),
    ("useTransition", Primitive::UseTransition),
];

/// Public aliases whose implementation is the canonical primitive itself.
/// The web entrypoint re-exports `createRenderEffect as effect`, so losing the
/// alias also loses the effect's owner and disposal obligations.
const ALIASES: &[(&str, Primitive)] = &[("effect", Primitive::CreateRenderEffect)];

/// Every name this dialect exports, derived from [`TABLE`] rather than
/// mirrored beside it. The mirror was a second list to keep in step, and
/// keeping two lists in step by hand is the defect this crate exists to
/// remove one level down.
#[cfg(test)]
pub(crate) fn names() -> Vec<&'static str> {
    TABLE.iter().chain(ALIASES).map(|(name, _)| *name).collect()
}

impl Dialect for Solid1x {
    fn version(&self) -> Version {
        Version::V1
    }

    fn component_name_may_be_component(&self, name: &str) -> bool {
        name.as_bytes()
            .first()
            .is_some_and(|byte| !byte.is_ascii_lowercase())
    }

    /// 1.x ships four user-facing subpaths, and the distinction is load
    /// bearing: `createStore` imported from `solid-js` is wrong in 1.x and
    /// correct in 2.0.
    fn modules(&self) -> &'static [&'static str] {
        &[
            "solid-js",
            "solid-js/store",
            "solid-js/web",
            "solid-js/universal",
        ]
    }

    /// 1.x defines every primitive inside the one `solid-js` package; the
    /// store, web and universal vocabularies are its subpaths, not separate
    /// archives.
    fn primitive_defining_packages(&self) -> &'static [&'static str] {
        &["solid-js"]
    }

    /// Reviewed Solid 1 semantics in this module, not a package certificate.
    fn runtime_model_identity(&self) -> &'static str {
        "solid-v1/model-1"
    }

    /// [`AUDITED_ARCHIVES`] and [`NEGATIVE_ROWS`] — sixteen `creates` denials
    /// for `solid-js@1.9.14`, all read out of the archive's own runtime bytes
    /// by hand ([`SOLID1_CORE_PRIMITIVES_AUDIT`]); see [`NEGATIVE_ROWS`] for
    /// why the solid-v1 JSON documents contribute no row.
    fn negative_claim_authority(&self) -> &'static DialectNegativeAuthority {
        &NEGATIVE_AUTHORITY
    }

    fn primitive(&self, name: &str) -> Option<Primitive> {
        static INDEX: crate::NameIndex = crate::NameIndex::new();
        lookup(&INDEX, &[TABLE, ALIASES], name)
    }

    fn name_of(&self, primitive: Primitive) -> Option<&'static str> {
        reverse(TABLE, primitive)
    }

    /// Source: the signatures quoted in `docs/solid-1x-api-surface.md`.
    ///
    /// `createEffect<Next, Init>(fn, value?, options?)` — the callback is at
    /// index 0 and **index 1 is a seed value, not a callback**. This is the
    /// difference ADR 0001 led with, inverted: reading 1.x's seed as 2.0's
    /// apply callback is the highest-yield mistake available here.
    ///
    /// The engine once made this exact mistake (a pre-vocabulary site read
    /// the seed value as the callback); wiring it onto this table fixed
    /// that, and this table is now the only place the answer lives.
    fn callback_positions(&self, primitive: Primitive) -> &'static [usize] {
        match primitive {
            Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateComputed
            | Primitive::CreateMemo
            | Primitive::CreateDeferred
            | Primitive::CreateDynamic
            | Primitive::CreateReaction
            | Primitive::CreateRoot
            | Primitive::Children
            | Primitive::Untrack
            | Primitive::Batch
            | Primitive::StartTransition
            | Primitive::OnMount
            | Primitive::OnCleanup
            | Primitive::OnError
            | Primitive::From
            | Primitive::Hydrate
            | Primitive::Lazy
            | Primitive::Render
            | Primitive::WebMemo
            | Primitive::Produce => &[0],
            Primitive::CreateSelector => &[0, 1],
            Primitive::CatchError | Primitive::On => &[0, 1],
            // createResource(source, fetcher) — the fetcher may sit at either
            // index depending on whether a source is supplied.
            Primitive::CreateResource => &[0, 1],
            // mapArray(list, mapFn) / indexArray(list, mapFn): the returned
            // accessor calls the list under tracking and the mapper under
            // `untrack`.
            Primitive::MapArray | Primitive::IndexArray => &[0, 1],
            // runWithOwner(owner, fn) / modifyMutable(state, modifier)
            Primitive::RunWithOwner | Primitive::ModifyMutable => &[1],
            _ => &[],
        }
    }

    /// Callbacks that explicitly clear tracking or run later outside their
    /// creating computation. Synchronous wrappers such as `batch`, `from`,
    /// and the protected body of `catchError` are absent because the 1.9.14
    /// runtime preserves `Listener` while invoking them.
    ///
    /// These roles are pinned to the published `solid-js@1.9.14` runtime and
    /// checked against the bundled package contract below. In particular,
    /// `lazy(loader)` stores `loader` on the returned component and invokes it
    /// only when that component (or its `preload` method) is called.
    fn runs_callback_deferred(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::Untrack
                | Primitive::CreateRoot
                | Primitive::CreateReaction
                | Primitive::OnMount
                | Primitive::OnCleanup
                | Primitive::OnError
                | Primitive::Lazy
                | Primitive::RunWithOwner
        )
    }

    /// 1.x's async boundary is `Suspense`; its error boundary is
    /// `ErrorBoundary`. There is no `Loading` and no `Errored` —
    /// `docs/solid-1x-api-surface.md` records `Errored` as a false positive,
    /// present only as the `"errored"` member of `createResource`'s state
    /// union.
    ///
    /// `SuspenseList` is deliberately absent. In the published 1.9.14 runtime
    /// only `Suspense` provides a `SuspenseContext`; `SuspenseList` registers
    /// a `SuspenseListContext` that coordinates the *reveal order* of child
    /// `Suspense` boundaries and bounds nothing itself. A pending read beneath
    /// a `SuspenseList` with no `Suspense` between them escapes to the nearest
    /// outer `Suspense`, exactly as if the list were not there.
    fn boundary_kind(&self, tag: &str) -> Option<Boundary> {
        match tag {
            "Suspense" => Some(Boundary::Async),
            "ErrorBoundary" => Some(Boundary::Error),
            _ => None,
        }
    }

    /// The 1.x shape of the same question `Solid2::callback_owners` answers.
    ///
    /// Two differences from 2.0, both consequences of the effect split not
    /// having happened yet:
    ///
    /// - `createEffect(fn, value?)` has one callback and it is owned. There is
    ///   no apply phase to run unowned, so unlike 2.0 there is no index 1
    ///   entry -- reading 1.x's seed value as 2.0's apply is the mistake
    ///   [`Solid1x::callback_positions`] exists to prevent, and repeating it
    ///   here would mark a seed value as an unowned callback.
    /// - `batch` and `startTransition` join `untrack` as callbacks that
    ///   inherit: they change how updates are scheduled, not who owns them.
    ///
    /// The published 1.9.14 runtime is the evidence for these owner roles.
    /// Higher-order helpers still require a call-site proof before their edge
    /// exists; an owner role never implies that a dormant callback ran.
    fn callback_owners(&self, primitive: Primitive) -> &'static [(usize, CallbackOwner)] {
        match primitive {
            Primitive::CreateRoot
            | Primitive::Children
            | Primitive::CreateMemo
            | Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateComputed
            | Primitive::CreateDeferred
            | Primitive::CreateDynamic => &[(0, CallbackOwner::Creates)],
            Primitive::CreateSelector => {
                &[(0, CallbackOwner::Creates), (1, CallbackOwner::Inherits)]
            }
            Primitive::WebMemo => &[(0, CallbackOwner::Creates)],
            Primitive::Hydrate | Primitive::Render => &[(0, CallbackOwner::Creates)],
            // catchError allocates a computation before invoking its protected
            // body. Its handler is deliberately absent: an immediate throw
            // runs it under that owner, while a queued reactive error runs it
            // under a synthetic error effect, so one flat owner role would
            // fabricate certainty.
            Primitive::CatchError => &[(0, CallbackOwner::Creates)],
            // The supplied owner is nullable. The call-site classifier
            // sharpens this to Creates or None when its value is proven.
            Primitive::RunWithOwner => &[(1, CallbackOwner::Conditional)],
            // The reaction owns its invalidation callback: runComputation
            // installs the reaction node as Owner, and cleanNode disposes the
            // callback's cleanups and children before the next run.
            Primitive::CreateReaction => &[(0, CallbackOwner::Creates)],
            // The flat package-contract form records the two-argument
            // fetcher. `callback_owner_at` supplies both overloads and the
            // tracked source's created owner; see it for why the fetcher's
            // owner is Conditional rather than None.
            Primitive::CreateResource => &[(1, CallbackOwner::Conditional)],
            Primitive::MapArray | Primitive::IndexArray => &[(1, CallbackOwner::Creates)],
            Primitive::ModifyMutable => &[(1, CallbackOwner::Inherits)],
            Primitive::Untrack
            | Primitive::Batch
            | Primitive::StartTransition
            | Primitive::From
            | Primitive::Lazy
            | Primitive::Produce => &[(0, CallbackOwner::Inherits)],
            Primitive::On => &[(0, CallbackOwner::Inherits), (1, CallbackOwner::Inherits)],
            _ => &[],
        }
    }

    /// `createResource`'s fetcher owner depends on which path invokes it,
    /// which is why both overloads answer [`CallbackOwner::Conditional`]
    /// rather than [`CallbackOwner::None`]. The published 1.9.14 runtime has
    /// three invocation paths:
    ///
    /// - **Sourced** `createResource(source, fetcher)` calls `load` from
    ///   inside the resource's internal `createComputed(() => load(false))`,
    ///   so the fetcher runs under that computation's owner.
    /// - **Unsourced** `createResource(fetcher)` runs its initial `load(false)`
    ///   synchronously during the `createResource` call itself, under
    ///   whatever owner the caller has.
    /// - **`refetch()`** may be called from any site — an event handler, a
    ///   timeout — and there `load` runs with no owner at all.
    ///
    /// So the fetcher usually runs owned and sometimes does not; only a
    /// call-site proof (this is the initial load, not a refetch) could
    /// sharpen the answer, and this table cannot carry one.
    fn callback_owner_at(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> Option<CallbackOwner> {
        // Every function-valued mergeProps source is wrapped in createMemo.
        // This is variadic and therefore cannot be represented by the static
        // callback_owners table's finite argument indices.
        if primitive == Primitive::MergeProps && argument < argument_count {
            return Some(CallbackOwner::Creates);
        }
        if primitive == Primitive::CreateResource {
            return match (argument_count, argument) {
                (1, 0) => Some(CallbackOwner::Conditional),
                (2.., 0) => Some(CallbackOwner::Creates),
                (2.., 1) => Some(CallbackOwner::Conditional),
                _ => None,
            };
        }
        self.callback_owners(primitive)
            .iter()
            .find(|(index, _)| *index == argument)
            .map(|(_, owner)| *owner)
    }

    /// Source: `solid-js@1.9.14`'s `types/render/flow.d.ts`, read from the
    /// installed package.
    ///
    /// ```text
    /// For<T>(props):   children: (item: T[number],           index: Accessor<number>) => U
    /// Index<T>(props): children: (item: Accessor<T[number]>, index: number)           => U
    /// ```
    ///
    /// Exact mirrors, and the engine knew only the first. `<Index>`'s item
    /// accessor was registered as a source nowhere, so a read of it was traced
    /// to nothing and reported by no rule.
    ///
    /// 1.x's `<For>` has no `keyed` prop — the three-way form is 2.0's — so
    /// the key shape does not change its answer.
    fn children_accessor_parameters(
        &self,
        primitive: Primitive,
        key: crate::KeyForm,
    ) -> &'static [usize] {
        match primitive {
            // Keyed <Show>/<Match> hand the callback the raw value; unkeyed
            // hand it an accessor. Both overloads are in flow.d.ts.
            //
            // `keyed` is a *boolean* prop in 1.x — the key-function form is
            // 2.0's — so `keyed={expr}` is a flag whose truthiness picks the
            // overload at runtime: truthy hands the callback the raw value,
            // falsy an accessor. A static table cannot prove which, and
            // claiming an accessor for a raw value would fabricate a source,
            // so both expression forms (proven-function and dynamic flag)
            // claim nothing.
            Primitive::Show | Primitive::Match => match key {
                crate::KeyForm::Keyed | crate::KeyForm::CustomKey | crate::KeyForm::DynamicFlag => {
                    &[]
                }
                crate::KeyForm::Unkeyed | crate::KeyForm::Absent => &[0],
            },
            Primitive::For => &[1],
            Primitive::Index => &[0],
            _ => &[],
        }
    }

    fn callback_accessor_parameters(
        &self,
        primitive: Primitive,
        argument: usize,
    ) -> &'static [usize] {
        match (primitive, argument) {
            (Primitive::MapArray, 1) => &[1],
            (Primitive::IndexArray, 1) => &[0],
            _ => &[],
        }
    }

    /// 1.x has two, and neither is one of 2.0's extras: `createStore` returns
    /// `[store, setStore]` and `createMutable` returns the store itself.
    /// `createProjection` and `createOptimisticStore` do not exist here, so
    /// the 2.0-shaped list the engine used was inert rather than wrong — but
    /// only by accident, and it named nothing 1.x-specific.
    fn returns_store(&self, primitive: Primitive) -> bool {
        matches!(primitive, Primitive::CreateStore | Primitive::CreateMutable)
    }

    /// Source: `solid-js@1.9.14/types/reactive/signal.d.ts`, read from the
    /// installed package. It declares
    /// `createSignal<T>(value: T, options?): Signal<T>` with
    /// `type Signal<T> = [get: Accessor<T>, set: Setter<T>]`, and
    /// `createMemo<Next extends Prev, Prev = Next>(fn): Accessor<Next>`.
    /// `Accessor` and `Setter` are the same
    /// public type exports [`Dialect::type_role`] already classifies, so the
    /// two rows restate an audited declaration rather than inferring one from a
    /// name.
    ///
    /// Deliberately only these two. `createResource`'s `[Resource<T>, {...}]`,
    /// `useTransition`'s `[Accessor<boolean>, ...]`, `createDeferred` and
    /// `createSelector` are all real 1.x accessor results, and every one of
    /// them stays absent until a proof needs it and its 2.0 counterpart has
    /// been reviewed: an unused row is an unaudited row that a later consumer
    /// would read as audited.
    fn reactive_result_slot(&self, primitive: Primitive, slot: ResultSlot) -> Option<ReactiveRole> {
        match (primitive, slot) {
            (Primitive::CreateSignal, ResultSlot::TupleItem(0)) => Some(ReactiveRole::Accessor),
            (Primitive::CreateSignal, ResultSlot::TupleItem(1)) => Some(ReactiveRole::Setter),
            (Primitive::CreateMemo, ResultSlot::Whole) => Some(ReactiveRole::Accessor),
            _ => None,
        }
    }

    /// Source: the declarations in `solid-js@1.9.14`, read from the installed
    /// package — `types/reactive/signal.d.ts` and `store/types/`.
    ///
    /// Every position 1.x shares with 2.0 is a coincidence, and three are not
    /// shared at all. `createMemo(fn, value?, options?)` puts options at 2
    /// where 2.0 puts them at 1; `createStore(store?, options?)` and
    /// `createMutable(state, options?)` put them at 1 where 2.0's derived
    /// forms put them at 2. The engine used 2.0's numbers for both dialects,
    /// so a 1.x `createMemo(fn, seed)` had its *seed* read as an options
    /// object — and the dialect fixture pair carried the resulting finding.
    ///
    /// `createComputed`, `createDeferred` and `createSelector` are here and
    /// were in no list before: 2.0 has none of them, so nothing 2.0-shaped
    /// could have had an opinion.
    ///
    /// `on(deps, fn, options?)` takes `{ defer }` at index 2. The slot is
    /// modelled because it is where the options are; the `defer` key itself
    /// is deliberately unmodelled. The engine's only options consumer today
    /// is sync-option detection, which every 1.x primitive opts out of via
    /// `supports_sync_option`, and `defer` is not a sync option — it makes
    /// the runtime skip `fn` until deps first *change*. Modelling that skip
    /// (the callback is dormant until a dependency writes) would need engine
    /// machinery for first-run reachability that does not exist yet; future
    /// work, not a table entry.
    fn options_argument(&self, primitive: Primitive) -> Option<usize> {
        match primitive {
            Primitive::CreateSignal
            | Primitive::CreateStore
            | Primitive::CreateMutable
            | Primitive::CreateDeferred => Some(1),
            Primitive::CreateMemo
            | Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateComputed
            | Primitive::CreateSelector
            | Primitive::On => Some(2),
            _ => None,
        }
    }

    /// Source: the checked Solid 1 normalized authority, held to it by
    /// `the_callback_executions_agree_with_the_bundled_contract`.
    ///
    /// The entry that matters most is `createEffect`: **one** tracked callback,
    /// at index 0. 1.x's second argument is a seed value threaded to the next
    /// run as `prev`, not 2.0's apply callback, and the engine described reads
    /// in it as being in an "apply callback" for as long as that pair was
    /// hardcoded.
    ///
    /// `createSignal` and `createStore` are absent, unlike 2.0's: 1.x has no
    /// derived form of either, so a function passed to them is a value.
    fn callback_executions(&self, primitive: Primitive) -> &'static [(usize, Execution)] {
        match primitive {
            Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateComputed
            | Primitive::CreateMemo
            | Primitive::Children
            | Primitive::CreateDynamic
            | Primitive::CreateDeferred
            | Primitive::WebMemo => &[(0, Execution::Tracked)],
            Primitive::CreateSelector => &[(0, Execution::Tracked), (1, Execution::Inline)],
            Primitive::CreateReaction
            | Primitive::Lazy
            | Primitive::OnCleanup
            | Primitive::OnMount
            | Primitive::OnError => &[(0, Execution::Deferred)],
            // createResource(source, fetcher): the fetcher runs outside the
            // creating computation, and its index depends on whether a source
            // was supplied.
            Primitive::CreateResource => &[(1, Execution::Deferred)],
            Primitive::CatchError => &[(0, Execution::Inline), (1, Execution::Deferred)],
            Primitive::CreateRoot
            | Primitive::Untrack
            | Primitive::Batch
            // With transitions enabled the 1.9 runtime invokes
            // startTransition's callback in a Promise.resolve().then()
            // microtask, not synchronously. `Inline` is the right
            // classification anyway: [`Execution`] classifies attribution, and
            // the runtime restores the captured Listener around the callback,
            // so reads inside it subscribe exactly as at the call site and it
            // never re-runs. Probed in
            // the receipt-issued Solid 1 normalized authority corpus.
            | Primitive::StartTransition
            | Primitive::From
            | Primitive::Hydrate
            | Primitive::Render
            | Primitive::Produce => &[(0, Execution::Inline)],
            Primitive::MapArray | Primitive::IndexArray => {
                &[(0, Execution::Tracked), (1, Execution::Deferred)]
            }
            // `on`'s deps run *inline* in whatever computation invokes the
            // returned adapter — deliberately not `Tracked`, although the
            // reads do subscribe there: the engine's returned-adapter
            // classifier keys on `Inline` to derive the role from the
            // concrete invocation site (`createEffect(on(...))` tracks, a
            // bare top-level adapter call does not). `mapArray`'s list stays
            // `Tracked` because its row computations re-read it themselves.
            Primitive::On => &[(0, Execution::Inline), (1, Execution::Deferred)],
            Primitive::RunWithOwner | Primitive::ModifyMutable => &[(1, Execution::Inline)],
            _ => &[],
        }
    }

    fn callback_requires_return_invocation(&self, primitive: Primitive, argument: usize) -> bool {
        (primitive == Primitive::CreateSelector && argument == 1)
            || (argument == 0
                && matches!(
                    primitive,
                    Primitive::CreateReaction | Primitive::Lazy | Primitive::Produce
                ))
            || (argument <= 1
                && matches!(
                    primitive,
                    Primitive::MapArray | Primitive::IndexArray | Primitive::On
                ))
    }

    /// The function arguments 1.x stores instead of invoking: a signal's
    /// initial value, and the `prev` seed threaded into effects, computeds,
    /// and memos. 2.0 answers differently for the first — `createSignal(fn)`
    /// there is a derived signal whose compute runs tracked. Source:
    /// `docs/solid-1x-api-surface.md`.
    fn stores_function_argument_as_value(&self, primitive: Primitive, argument: usize) -> bool {
        (primitive == Primitive::CreateSignal && argument == 0)
            || (argument == 1
                && matches!(
                    primitive,
                    Primitive::CreateEffect
                        | Primitive::CreateRenderEffect
                        | Primitive::CreateComputed
                        | Primitive::CreateMemo
                ))
    }

    fn returned_callback_execution_at(
        &self,
        primitive: Primitive,
        result_slot: Option<usize>,
        argument: usize,
        argument_count: usize,
    ) -> Option<Execution> {
        match (primitive, result_slot, argument, argument_count) {
            (Primitive::CreateReaction, None, 0, 1..) => Some(Execution::Tracked),
            // The transition starter shares startTransition's timing caveat:
            // the 1.9 runtime may invoke it in a Promise.resolve().then()
            // microtask, but it restores the captured Listener, so
            // tracking-wise it is the call site's scope — Inline.
            (Primitive::UseTransition, Some(1), 0, 1..) => Some(Execution::Inline),
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
            (Primitive::UseTransition, Some(1), 0, 1..) => Some(CallbackOwner::Inherits),
            _ => None,
        }
    }

    fn callback_execution_at(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> Option<Execution> {
        // `mergeProps(...sources)` checks every source and wraps each function
        // in `createMemo`. Keep this call-shape fact native because inferred
        // summaries support fixed callback parameters, not variadics.
        if primitive == Primitive::MergeProps && argument < argument_count {
            return Some(Execution::Tracked);
        }
        if primitive == Primitive::CreateResource {
            return match (argument_count, argument) {
                (1, 0) => Some(Execution::Deferred),
                (2.., 0) => Some(Execution::Tracked),
                (2.., 1) => Some(Execution::Deferred),
                _ => None,
            };
        }
        self.callback_executions(primitive)
            .iter()
            .find(|(index, _)| *index == argument)
            .map(|(_, execution)| *execution)
    }

    /// Read from `solid-js@1.9.14` `dist/solid.js`, the artifact the oracle
    /// install under `rust/target/tsc-oracle/v1` resolves for the client
    /// conditions — line numbers are that file's.
    ///
    /// Eager, all four via `updateComputation(c)` on the creating call:
    ///
    /// - `createMemo` (`:244-256`) — `updateComputation(c)` before it returns
    ///   `readSignal.bind(c)`;
    /// - `createRenderEffect` (`:218-221`) — the same line, no queue;
    /// - `createComputed` (`:214-217`) — likewise (contract emission cannot
    ///   currently reach it, because `primitive_callback_execution` has no
    ///   schedule row for it; the fact is still this dialect's to state);
    /// - `createResource`'s *source* (`:283`,
    ///   `dynamic = typeof source === "function" && createMemo(source)`) — a
    ///   memo, so eager for the same reason. Only the two-argument overload has
    ///   a tracked source; the one-argument form's single callback is the
    ///   fetcher, and [`Execution::Deferred`] is not this method's domain;
    /// - `mergeProps` (`:1329`,
    ///   `sources[i] = typeof s === "function" ? (proxy = true, createMemo(s)) : s`)
    ///   — every function-valued source becomes a memo, at every index.
    ///
    /// Deferring: `createEffect` (`:222-229`,
    /// `Effects ? Effects.push(c) : updateComputation(c)`). The push branch is
    /// the one an export takes: `createRoot` runs its body through
    /// `runUpdates(updateFn, true)` (`:192`), which installs `Effects = []`
    /// (`:820`), so under any owner — and a package export that creates an
    /// effect has one — the computation runs from `completeUpdates` after the
    /// creating call returned. This is what makes 1.x's own `onMount`
    /// (`createEffect(() => untrack(fn))`) `deferred` rather than `inline`.
    ///
    /// One documented caveat on the eager four: each is written
    /// `if (Scheduler && Transition && Transition.running) Updates.push(c); else
    /// updateComputation(c)`. `Scheduler` is null until an application calls
    /// `enableScheduling()`, and the audited default configuration — the one the
    /// probe measures and the one a contract describes — never installs one, so
    /// the else branch is unconditional there. A contract cannot describe both
    /// configurations with one word; it describes the default.
    ///
    /// Everything else is deliberately absent. 1.x's `createSignal(fn)` and
    /// `createStore(fn)` never invoke the argument at all
    /// ([`Dialect::stores_function_argument_as_value`] states the first), so no
    /// schedule is honest for them, and `children`/`createDeferred`/
    /// `createSelector`/`createDynamic`/`mapArray` have no schedule row in
    /// `primitive_callback_execution` either.
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
            | Primitive::CreateRenderEffect
            | Primitive::CreateComputed
            | Primitive::CreateResource
            | Primitive::MergeProps => Some(TrackedCallbackTiming::DuringCall),
            Primitive::CreateEffect => Some(TrackedCallbackTiming::AfterCall),
            _ => None,
        }
    }

    fn reports_untracked_reads_at(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> bool {
        (matches!(
            primitive,
            Primitive::Hydrate | Primitive::Lazy | Primitive::Render
        ) && argument == 0)
            || (primitive == Primitive::CreateResource
                && matches!((argument_count, argument), (1, 0) | (2.., 1)))
            || (matches!(
                primitive,
                Primitive::MapArray | Primitive::IndexArray | Primitive::RunWithOwner
            ) && argument == 1)
    }

    /// `<Ctx.Provider value={...}>` — the value getter runs untracked.
    fn context_provider_member(&self) -> Option<&'static str> {
        Some("Provider")
    }

    /// The `v1/` catalog carries the SC8xxx ESLint-era surface.
    fn carries_eslint_era_rules(&self) -> bool {
        true
    }

    fn static_event_values_are_attributes(&self) -> bool {
        true
    }

    /// `Suspense`, not `Loading`; `ErrorBoundary`, not `Errored`. Source:
    /// `docs/solid-1x-api-surface.md`, the control-flow components.
    ///
    /// A total inverse of [`Solid1x::boundary_kind`]: `SuspenseList` is in
    /// neither, because it coordinates child `Suspense` boundaries' reveal
    /// order without providing a `SuspenseContext` of its own.
    fn boundary_name(&self, boundary: Boundary) -> &'static str {
        match boundary {
            Boundary::Async => "Suspense",
            Boundary::Error => "ErrorBoundary",
        }
    }

    /// The 2.0 rule minus the primitives 1.x does not have, plus 1.x's own
    /// owner-creating primitives. State factories are absent: a function
    /// passed to `createSignal`, `createStore`, or `createMutable` is stored as
    /// data in 1.x and registers no work in the surrounding owner.
    fn cleanup_rule(&self, primitive: Primitive) -> CleanupRule {
        match primitive {
            Primitive::OnCleanup
            | Primitive::OnMount
            | Primitive::CreateMemo
            | Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateComputed
            | Primitive::CreateDeferred
            | Primitive::CreateDynamic
            | Primitive::CreateSelector
            // createReaction allocates a computation the moment it is called
            // (1.9.14 builds one with `createComputation` and reuses it for
            // every `track`), so like createEffect it needs a surrounding
            // owner to dispose the reaction itself. Its invalidation callback
            // separately runs under the reaction's own created owner.
            | Primitive::CreateReaction
            | Primitive::CreateRoot
            | Primitive::MapArray
            | Primitive::IndexArray
            | Primitive::From
            | Primitive::Hydrate
            | Primitive::Render
            | Primitive::WebMemo
            // createResource eagerly creates computations (a render effect
            // when a source is supplied) that need disposal by their
            // surrounding owner.
            | Primitive::CreateResource
            | Primitive::Children => CleanupRule::Always,
            _ => CleanupRule::Never,
        }
    }

    /// Nothing. Returning a cleanup is a 2.0 idea.
    ///
    /// 1.x declares its effect callbacks as
    /// `EffectFunction<Prev, Next> = (v: Prev) => Next` and threads the return
    /// value to the next run, so `createEffect(prev => prev + 1, 0)` is
    /// idiomatic accumulation. `createReaction`'s `onInvalidate` and
    /// `onMount`'s callback are both declared `() => void`. Cleanup in 1.x is
    /// `onCleanup` and only `onCleanup`.
    ///
    /// The shared list this replaced named `createEffect`,
    /// `createRenderEffect` and `createReaction` — all three real in 1.x — so
    /// the accumulating form was reported as an unprovable cleanup return.
    fn accepts_cleanup_return(&self, _primitive: Primitive) -> bool {
        false
    }

    /// 1.x renders `<Index each>{item => ...}</Index>` the same way 2.0
    /// renders `<Repeat>`. The shared list the engine used named `Repeat`,
    /// which 1.x does not have, and omitted `Index`, which it does — so a
    /// function written inside an `<Index>` was read as a component.
    fn renders_children_through_callback(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::For
                | Primitive::Index
                | Primitive::Show
                | Primitive::Match
                | Primitive::Switch
        )
    }

    /// 1.x's own set, and the reason this is a dialect question at all.
    /// `createResource` returns `[accessor, { mutate, refetch }]`,
    /// `useTransition` returns `[pendingAccessor, startTransition]`, and
    /// `createMutable` returns a store. The two tuple shapes do not fit the
    /// bundled contract's single-value `returns` column — so native runtime
    /// facts must identify their reactive slots.
    ///
    /// `createComputed` is absent on purpose: it returns nothing.
    fn creates_reactive_source(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateSignal
                | Primitive::Children
                | Primitive::CreateMemo
                | Primitive::CreateDynamic
                | Primitive::CreateStore
                | Primitive::CreateMutable
                | Primitive::CreateResource
                | Primitive::CreateDeferred
                | Primitive::CreateSelector
                | Primitive::From
                | Primitive::WebMemo
                | Primitive::UseTransition
        )
    }

    /// 1.x spells it `mergeProps`, exported from `solid-js`. `splitProps`
    /// is deliberately absent: it returns a *tuple* of proxies, so the root
    /// travels through array destructuring rather than through the call's own
    /// value, and claiming it here would name the tuple reactive.
    fn merges_props_reactivity(&self, primitive: Primitive) -> bool {
        primitive == Primitive::MergeProps
    }

    /// 1.x spells it `splitProps`.
    fn splits_props(&self, primitive: Primitive) -> bool {
        primitive == Primitive::SplitProps
    }

    /// `createSignal` returns `[get, set]`, `createStore` returns
    /// `[store, setStore]`, and `createResource` returns
    /// `[accessor, { mutate, refetch }]`. `createMutable` returns the store
    /// itself, so it is deliberately absent.
    fn returns_reactive_tuple(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateSignal | Primitive::CreateStore | Primitive::CreateResource
        )
    }

    fn creates_directive_owner(&self, _primitive: Primitive) -> bool {
        // Solid 1.x applies directives and refs through untrack(), which
        // clears Listener but preserves Owner. Nothing created there enters
        // the unowned directive-leak class modeled by SC6001 in Solid 2.0.
        false
    }

    /// Source: `docs/solid-1x-api-surface.md`, the `solid-js` and
    /// `solid-js/store` sections. The split is the difference that makes
    /// module gating worth having: every name below sits in core in 2.0.
    /// Straight out of the generated index: 1.x ships one package with four
    /// subpaths, so one table answers for all of them.
    fn export_modules(&self, name: &str, position: crate::ExportPosition) -> Vec<&'static str> {
        crate::exports::modules(
            crate::exports::solid_v1_solid_js::VALUES,
            crate::exports::solid_v1_solid_js::TYPES,
            name,
            position,
        )
    }

    /// Per-subpath, which is the whole reason this is module-scoped:
    /// `solid-js/store` exposes the store primitives and core does not.
    ///
    /// The checked-in package export census is the source of truth. The unit
    /// test below derives the expected names from that census and [`TABLE`],
    /// so adding a modelled obligation without its namespace spelling fails.
    fn namespace_import_primitives(&self, module: &str) -> &'static [&'static str] {
        match module {
            "solid-js" => NAMESPACE_CORE,
            "solid-js/store" => NAMESPACE_STORE,
            "solid-js/web" => NAMESPACE_WEB,
            _ => &[],
        }
    }
}

const NAMESPACE_CORE: &[&str] = &[
    "ErrorBoundary",
    "For",
    "Index",
    "Match",
    "Show",
    "Suspense",
    "SuspenseList",
    "Switch",
    "batch",
    "catchError",
    "children",
    "createComputed",
    "createContext",
    "createDeferred",
    "createEffect",
    "createMemo",
    "createReaction",
    "createRenderEffect",
    "createResource",
    "createRoot",
    "createSelector",
    "createSignal",
    "from",
    "getOwner",
    "indexArray",
    "lazy",
    "mapArray",
    "mergeProps",
    "on",
    "onCleanup",
    "onError",
    "onMount",
    "runWithOwner",
    "splitProps",
    "startTransition",
    "untrack",
    "useContext",
    "useTransition",
];

const NAMESPACE_STORE: &[&str] = &[
    "createStore",
    "produce",
    "reconcile",
    "unwrap",
    "createMutable",
    "modifyMutable",
];

const NAMESPACE_WEB: &[&str] = &[
    "ErrorBoundary",
    "For",
    "Index",
    "Match",
    "Show",
    "Suspense",
    "SuspenseList",
    "Switch",
    "createDynamic",
    "effect",
    "getOwner",
    "hydrate",
    "memo",
    "mergeProps",
    "render",
    "untrack",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn repository_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .canonicalize()
            .unwrap()
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(bytes))
    }

    /// Where the checked-in copy of a cited byte range lives; derived from the
    /// citation's own fields so a row and its slice cannot be named
    /// inconsistently. Mirrors `solid_2`'s layout under `solid-v1/`.
    fn audited_slice_path(archive_path: &str, start_byte: usize, end_byte: usize) -> String {
        format!(
            "rust/crates/solid-dialect/audited-slices/solid-v1/solid-js/{archive_path}.{start_byte}-{end_byte}.slice"
        )
    }

    /// Every row cites bytes that say what the row says — for the
    /// `Implementation` kind, the *subject* of the human reading: the audit
    /// document and its section exist, the cited file is the pinned file at
    /// the pinned digest, the range fits, the checked-in slice hashes to the
    /// cited digest and begins with the export's own definition, and, when the
    /// archive is on disk, the slice is still exactly those bytes of it.
    /// Nothing here re-derives closure from a JavaScript body (ADR 0007).
    #[test]
    fn every_negative_row_citation_resolves_to_the_bytes_it_claims() {
        let root = repository_root();
        assert!(!NEGATIVE_ROWS.is_empty());
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(
                root.join("benchmarks/package-contract-v2/phase0/solid-1x/solid-js/files.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let archive_root = std::env::var("SOLID_CHECKER_SOLID1_ARCHIVE_ROOT")
            .ok()
            .filter(|value| !value.is_empty());
        assert!(
            archive_root.is_some()
                || std::env::var("SOLID_CHECKER_EXPECT_PROBE_PINS").as_deref() != Ok("1"),
            "SOLID_CHECKER_EXPECT_PROBE_PINS=1, but SOLID_CHECKER_SOLID1_ARCHIVE_ROOT is unset: \
             the cited ranges would only be checked against the checked-in slices, never \
             against the archive they claim to quote. scripts/verify.sh exports it from the \
             tsc-oracle v1 install."
        );
        for row in NEGATIVE_ROWS {
            assert_eq!(row.package, "solid-js");
            assert_eq!(
                row.citations.len(),
                6,
                "{}: one citation per bundle",
                row.export
            );
            for citation in row.citations {
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
                    panic!("{}: every 1.x row is implementation-audited", row.export)
                };
                let audit_text = std::fs::read_to_string(root.join(audit)).unwrap();
                assert!(
                    audit_text.lines().any(|line| line == section),
                    "{}: {audit} does not contain {section:?} verbatim",
                    row.export
                );
                let entry = manifest
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|entry| entry["path"].as_str() == Some(archive_path))
                    .unwrap_or_else(|| {
                        panic!("{}: files.json does not pin {archive_path}", row.export)
                    });
                assert_eq!(
                    entry["sha256"].as_str(),
                    Some(file_sha256),
                    "{}: {archive_path}",
                    row.export
                );
                let file_bytes = entry["bytes"].as_u64().unwrap() as usize;
                assert!(
                    start_byte < end_byte && end_byte <= file_bytes,
                    "{}: {archive_path}",
                    row.export
                );
                let slice_path = audited_slice_path(archive_path, start_byte, end_byte);
                let slice = std::fs::read(root.join(&slice_path)).unwrap_or_else(|error| {
                    panic!("{}: no slice at {slice_path}: {error}", row.export)
                });
                assert_eq!(slice.len(), end_byte - start_byte, "{slice_path}");
                assert_eq!(sha256_hex(&slice), slice_sha256, "{slice_path}");
                let text = String::from_utf8(slice.clone()).unwrap();
                assert!(
                    text.starts_with(&format!("function {}(", row.export))
                        || text.starts_with(&format!("const {} = ", row.export)),
                    "{slice_path} does not begin with {}'s definition: {:?}",
                    row.export,
                    &text[..text.len().min(60)]
                );
                if let Some(archive_root) = &archive_root {
                    let file = std::path::Path::new(archive_root)
                        .join(row.package)
                        .join(archive_path);
                    let bytes = std::fs::read(&file).unwrap_or_else(|error| {
                        panic!(
                            "SOLID_CHECKER_SOLID1_ARCHIVE_ROOT is set, but {}: {error}",
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
                        "{}: {slice_path}",
                        row.export
                    );
                }
            }
        }
    }

    /// The archive tuple is the one every audited solid-v1 document names,
    /// and the pinned manifest agrees with it.
    #[test]
    fn the_audited_archive_is_the_bundled_documents_package_block() {
        let root = repository_root();
        let [archive] = AUDITED_ARCHIVES else {
            panic!("one archive")
        };
        let manifest_bytes = std::fs::read(
            root.join("benchmarks/package-contract-v2/phase0/solid-1x/solid-js/package.json"),
        )
        .unwrap();
        assert_eq!(sha256_hex(&manifest_bytes), archive.manifest_sha256);
        let mut documents = 0;
        for entry in std::fs::read_dir(root.join("pkg/contracts/bundled/solid-v1")).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().and_then(|name| name.to_str()) == Some("bundle-index.json") {
                continue;
            }
            let document: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            if document["package"]["name"].as_str() != Some(archive.name) {
                continue;
            }
            documents += 1;
            assert_eq!(
                document["package"]["version"].as_str(),
                Some(archive.version),
                "{}",
                path.display()
            );
            assert_eq!(
                document["package"]["integrity"].as_str(),
                Some(archive.integrity),
                "{}",
                path.display()
            );
            assert_eq!(
                document["package"]["manifest"]["sha256"].as_str(),
                Some(archive.manifest_sha256),
                "{}",
                path.display()
            );
        }
        assert!(documents >= 3, "the solid-js documents were not found");
    }

    /// Sorted by `(package, export, domain)`, no duplicates, canonical
    /// spellings only, one domain admitted, and nothing derived from the
    /// solid-v1 JSON documents' migrated closures.
    #[test]
    fn negative_rows_are_sorted_unique_canonical_and_implementation_only() {
        use std::collections::BTreeSet;
        let keys: Vec<_> = NEGATIVE_ROWS
            .iter()
            .map(|row| (row.package, row.export, row.domain))
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(keys, sorted, "rows must be sorted and unique");
        for row in NEGATIVE_ROWS {
            assert_eq!(
                Solid1x
                    .primitive(row.export)
                    .and_then(|primitive| Solid1x.name_of(primitive)),
                Some(row.export),
                "{} is not a canonical 1.x spelling",
                row.export
            );
            assert!(
                AUDITED_ARCHIVES
                    .iter()
                    .any(|archive| archive.name == row.package),
                "{}: row names an archive the dialect did not audit",
                row.package
            );
        }
        let admitted: BTreeSet<CallClaimDomain> =
            NEGATIVE_ROWS.iter().map(|row| row.domain).collect();
        assert_eq!(
            admitted,
            BTreeSet::from([CallClaimDomain::Creates]),
            "a new domain was added to the table without widening this comparison"
        );
        assert_eq!(
            NEGATIVE_ROWS.len(),
            16,
            "R1-R16 of the 1.x audit, and nothing else"
        );
        assert!(
            NEGATIVE_ROWS.iter().all(|row| row
                .citations
                .iter()
                .all(|citation| matches!(citation, AuditedCitation::Implementation { .. }))),
            "no 1.x row may cite a solid-v1 JSON summary: those closures are the migration's, not an audit's"
        );
    }

    /// The tier answers for these rows exactly as the census will ask it.
    #[test]
    fn the_authority_denies_creates_for_the_sixteen_and_nothing_else() {
        let [archive] = AUDITED_ARCHIVES else {
            panic!("one archive")
        };
        for export in [
            "batch",
            "createComputed",
            "createContext",
            "createEffect",
            "createMemo",
            "createRenderEffect",
            "createSignal",
            "getOwner",
            "mapArray",
            "mergeProps",
            "on",
            "onCleanup",
            "onMount",
            "splitProps",
            "untrack",
            "useContext",
        ] {
            assert!(
                crate::primitive_performs_no_operation(archive, export, CallClaimDomain::Creates),
                "{export} creates"
            );
            assert!(
                !crate::primitive_performs_no_operation(archive, export, CallClaimDomain::Reads),
                "{export} reads must stay silent"
            );
        }
        for export in [
            "createResource",
            "createRoot",
            "lazy",
            "createComponent",
            "indexArray",
            "children",
        ] {
            assert!(
                !crate::primitive_performs_no_operation(archive, export, CallClaimDomain::Creates),
                "{export} was not audited and must stay silent"
            );
        }
        // A same-named archive at another identity is silence.
        let other = AuditedArchive {
            version: "1.9.13",
            ..*archive
        };
        assert!(!crate::primitive_performs_no_operation(
            &other,
            "useContext",
            CallClaimDomain::Creates
        ));
    }

    #[test]
    fn compatibility_names_signal_possible_components_without_proving_them() {
        for component in ["App", "_App", "$App", "画面"] {
            assert!(Solid1x.component_name_may_be_component(component));
        }
        for helper in ["app", "zView"] {
            assert!(!Solid1x.component_name_may_be_component(helper));
        }
    }

    #[test]
    fn function_values_are_not_callbacks_or_owned_computations() {
        let dialect = Solid1x;
        for primitive in [
            Primitive::CreateSignal,
            Primitive::CreateStore,
            Primitive::CreateMutable,
        ] {
            assert!(dialect.callback_positions(primitive).is_empty());
            assert!(dialect.callback_executions(primitive).is_empty());
            assert!(dialect.callback_owners(primitive).is_empty());
            assert_eq!(dialect.callback_execution_at(primitive, 0, 1), None);
            assert_eq!(dialect.callback_owner_at(primitive, 0, 1), None);
            assert_eq!(dialect.cleanup_rule(primitive), CleanupRule::Never);
        }
    }

    #[test]
    fn catch_error_protected_body_has_a_created_owner_but_the_handler_stays_unknown() {
        assert_eq!(
            Solid1x.callback_owner_at(Primitive::CatchError, 0, 2),
            Some(CallbackOwner::Creates)
        );
        assert_eq!(
            Solid1x.callback_owner_at(Primitive::CatchError, 1, 2),
            None,
            "the handler may run immediately under the catch owner or later under the runtime's synthetic error effect"
        );
    }

    #[test]
    fn merge_props_function_sources_are_variadic_tracked_computations() {
        for argument in 0..3 {
            assert_eq!(
                Solid1x.callback_execution_at(Primitive::MergeProps, argument, 3),
                Some(Execution::Tracked)
            );
            assert_eq!(
                Solid1x.callback_owner_at(Primitive::MergeProps, argument, 3),
                Some(CallbackOwner::Creates)
            );
        }
        assert_eq!(
            Solid1x.callback_execution_at(Primitive::MergeProps, 3, 3),
            None
        );
        assert_eq!(Solid1x.callback_owner_at(Primitive::MergeProps, 3, 3), None);
    }

    #[test]
    fn returned_callback_contracts_keep_tuple_slots_and_owner_roles() {
        assert_eq!(
            Solid1x.returned_callback_execution_at(Primitive::UseTransition, Some(0), 0, 1),
            None,
            "the pending accessor is not the transition starter"
        );
        assert_eq!(
            Solid1x.returned_callback_execution_at(Primitive::UseTransition, Some(1), 0, 1),
            Some(Execution::Inline)
        );
        assert_eq!(
            Solid1x.returned_callback_owner_at(Primitive::UseTransition, Some(1), 0, 1),
            Some(CallbackOwner::Inherits)
        );
        assert_eq!(
            Solid1x.returned_callback_owner_at(Primitive::CreateReaction, None, 0, 1),
            Some(CallbackOwner::Creates)
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
            assert_eq!(reverse(TABLE, *primitive), Some(*name));
        }
    }

    #[test]
    fn suspense_list_is_not_an_async_boundary() {
        // SuspenseList only coordinates the reveal order of child Suspense
        // boundaries -- the 1.9.14 runtime gives it no SuspenseContext, so a
        // pending read beneath one (with no Suspense in between) is NOT
        // bounded by it. The name stays in the vocabulary as a component; it
        // just opens no boundary.
        assert_eq!(Solid1x.boundary_kind("SuspenseList"), None);
        assert_eq!(
            Solid1x.primitive("SuspenseList"),
            Some(Primitive::SuspenseList)
        );
    }

    #[test]
    fn keyed_is_a_boolean_prop_so_the_expression_form_claims_no_accessor() {
        for primitive in [Primitive::Show, Primitive::Match] {
            // Unkeyed (or absent) hands the children callback an accessor.
            assert_eq!(
                Solid1x.children_accessor_parameters(primitive, crate::KeyForm::Absent),
                &[0]
            );
            assert_eq!(
                Solid1x.children_accessor_parameters(primitive, crate::KeyForm::Unkeyed),
                &[0]
            );
            // Keyed hands it the raw value.
            assert!(
                Solid1x
                    .children_accessor_parameters(primitive, crate::KeyForm::Keyed)
                    .is_empty()
            );
            // `keyed={expr}` is a boolean flag in 1.x, not 2.0's key
            // function: truthy picks the raw-value overload, so claiming an
            // accessor here would fabricate a source over a raw value.
            assert!(
                Solid1x
                    .children_accessor_parameters(primitive, crate::KeyForm::CustomKey)
                    .is_empty()
            );
            assert!(
                Solid1x
                    .children_accessor_parameters(primitive, crate::KeyForm::DynamicFlag)
                    .is_empty()
            );
        }
    }

    #[test]
    fn the_store_primitives_are_recognized_but_gated_by_module() {
        // createStore exists in 1.x only under solid-js/store. The name
        // resolves here; whether the import path was legal is the contract
        // layer's question, not the vocabulary's.
        assert_eq!(
            Solid1x.primitive("createStore"),
            Some(Primitive::CreateStore)
        );
        assert!(Solid1x.owns_module("solid-js/store"));
    }

    /// Published 1.9.14 callback-bearing exports intentionally left to another
    /// evidence domain. `createComponent`, `use`, `insert`, and
    /// `getNextElement` are compiler/renderer protocols; the SSR/streaming
    /// helpers vary by package condition; the scheduler/external-source/
    /// observable/renderer hooks have nested or subscription-dependent
    /// contracts the flat dialect table cannot truthfully collapse.
    const UNMODELLED_CALLBACK_TAKERS: &[&str] = &[
        "createComponent",
        "createRenderer",
        "enableExternalSource",
        "enableScheduling",
        "getNextElement",
        "insert",
        "observable",
        "pipeToNodeWritable",
        "pipeToWritable",
        "renderToStream",
        "renderToString",
        "renderToStringAsync",
        "requestCallback",
        "use",
        "useAssets",
    ];

    /// Reviewed against the public declarations in `solid-js@1.9.14`'s
    /// `types/`, `store/types/`, `web/types/`, and `universal/types/` trees.
    /// This includes callback-valued JSX props and returned tuple functions,
    /// not only callbacks already present in the flat package contract.
    const PUBLISHED_CALLBACK_TAKERS: &[&str] = &[
        "ErrorBoundary",
        "For",
        "Index",
        "Match",
        "Show",
        "Switch",
        "batch",
        "catchError",
        "children",
        "createComponent",
        "createComputed",
        "createDynamic",
        "createEffect",
        "createMemo",
        "createReaction",
        "createRenderEffect",
        "createRenderer",
        "createResource",
        "createRoot",
        "createSelector",
        "effect",
        "enableExternalSource",
        "enableScheduling",
        "from",
        "getNextElement",
        "hydrate",
        "indexArray",
        "insert",
        "lazy",
        "mapArray",
        "memo",
        "modifyMutable",
        "observable",
        "on",
        "onCleanup",
        "onError",
        "onMount",
        "pipeToNodeWritable",
        "pipeToWritable",
        "produce",
        "render",
        "renderToStream",
        "renderToString",
        "renderToStringAsync",
        "requestCallback",
        "runWithOwner",
        "startTransition",
        "untrack",
        "use",
        "useAssets",
        "useTransition",
    ];

    #[test]
    fn every_callback_taking_export_is_modelled_or_excluded() {
        let exports = crate::callback_exports_from_bundles("solid-v1", &["solid-js"]);
        let mut unmodelled = exports
            .iter()
            .filter(|(_, callbacks)| !callbacks.is_empty())
            .filter_map(|(name, _)| {
                (Solid1x.primitive(name).is_none()
                    && !UNMODELLED_CALLBACK_TAKERS.contains(&name.as_str()))
                .then_some(name.clone())
            })
            .collect::<Vec<_>>();
        unmodelled.sort_unstable();
        assert!(
            unmodelled.is_empty(),
            "Solid 1.x exports declaring callbacks that the vocabulary does not model: {unmodelled:?}"
        );

        for name in UNMODELLED_CALLBACK_TAKERS {
            assert!(
                exports.contains_key(*name),
                "{name} is excluded but is not an export of solid-js any more"
            );
            assert!(
                Solid1x.primitive(name).is_none(),
                "{name} is both excluded and modelled"
            );
        }
    }

    #[test]
    fn every_published_callback_taker_is_modelled_or_has_a_reviewed_exclusion() {
        for name in PUBLISHED_CALLBACK_TAKERS {
            assert!(
                !Solid1x
                    .export_modules(name, crate::ExportPosition::Value)
                    .is_empty(),
                "{name} is in the reviewed callback census but is not a published value export"
            );
            assert!(
                Solid1x.primitive(name).is_some() || UNMODELLED_CALLBACK_TAKERS.contains(name),
                "{name} is callback-bearing but neither modelled nor explicitly excluded"
            );
        }
        for name in UNMODELLED_CALLBACK_TAKERS {
            assert!(
                PUBLISHED_CALLBACK_TAKERS.contains(name),
                "{name} is excluded without appearing in the reviewed callback census"
            );
            assert!(
                Solid1x.primitive(name).is_none(),
                "{name} is both explicitly excluded and modelled"
            );
        }
    }

    #[test]
    fn every_modelled_export_resolves_through_its_namespace_module() {
        for module in Solid1x.modules() {
            let mut expected = TABLE
                .iter()
                .chain(ALIASES)
                .filter_map(|(name, _)| {
                    Solid1x
                        .export_modules(name, crate::ExportPosition::Value)
                        .contains(module)
                        .then_some(*name)
                })
                .collect::<Vec<_>>();
            expected.sort_unstable();
            let mut actual = Solid1x.namespace_import_primitives(module).to_vec();
            actual.sort_unstable();
            assert_eq!(
                actual, expected,
                "namespace imports from {module} must retain every modelled runtime obligation"
            );
        }
    }
}
