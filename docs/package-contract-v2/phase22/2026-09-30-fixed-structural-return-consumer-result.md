# Published fixed-tuple return: certification and consumer result

Date: 2026-09-30; delivery checks completed 2026-10-01 JST.
Scope: ADR 0172's first complete published export and the integrated checkpoint.

`@solid-primitives/vibrate@1.0.0-next.2:frequencyToPattern` certifies and admits
independently for none, browser and node. Each issued stable-v1 document closes
callbacks, reads, creates and returns, and the returned tuple closes `items`
over exactly two `plain` members. No closure for this export is withheld.

Published package integrity:
`sha512-ni5srTKSzfwZTZ5Sq3SikOgTNCgSvqBgCh7PxclXrf7OeM+UYPybWju7SIYMDkThaW3x67ebf5hLdAikqwx9Iw==`.
All three trials select `./dist/index.js` through `/exports/./import/default`,
runtime SHA-256
`1cfdac6a285173d42c7b3d47f52372755b919def2d7c56bf7501a8e93303abf1`,
and the published `./dist/index.d.ts`, SHA-256
`fe720c183c7182125dfd64a4e29bb71dc013da693fac2c33ff8b35a558b9e8f2`.
Host-specific dependency closures are proved separately.

| Host | Issued main SHA-256 | Correct-use findings | Misuse findings |
| --- | --- | --- | --- |
| none | `d1fd9e575a93de42b197e4b19d692e13efdef6248e753b536cb888538e84aae1` | 0, certified | SC1001, violation |
| browser | `bfc4dd28c12f8f5fbb49e9cef6f5fe77c7d69b77fe81be092b98cc6e3e38d723` | 0, certified | SC1001, violation |
| node | `ae2651963aaf8553179c9e895e176704cff15ca35745bd955b6cbccd2bcceb58` | 0, certified | SC1001, violation |

The misuse snapshots a reactive input outside tracking:

```tsx
import { createOptimistic } from "solid-js";
import { frequencyToPattern } from "@solid-primitives/vibrate";

export default function App() {
  const [frequency] = createOptimistic(1);
  const pattern = frequencyToPattern(frequency());
  return <p>{pattern.join(",")}</p>;
}
```

The correct-use twin tracks that conversion:

```tsx
import { createMemo, createOptimistic } from "solid-js";
import { frequencyToPattern } from "@solid-primitives/vibrate";

export default function App() {
  const [frequency] = createOptimistic(1);
  const pattern = createMemo(() => frequencyToPattern(frequency()));
  return <p>{pattern().join(",")}</p>;
}
```

All six examples pass TypeScript 5.9.3 against the actual installed published
Solid/signals/web rc.9 and vibrate declarations. Configuration uses strict
checking, bundler resolution, renderer-owned `jsxImportSource: @solidjs/web`,
and `skipLibCheck` to exclude published declaration-file defects while checking
the consumers. Browser/node set the matching custom condition and checker
runtime condition. Catalog admission uses the exact canonical consumer path,
its installed dependency environment and each trial's receipt trust configuration.

Retained trial roots are
`/private/tmp/solid-checker-structures-vibrate-zL3rpm` (none),
`/private/tmp/solid-checker-structures-vibrate-BriHT9` (browser), and
`/private/tmp/solid-checker-structures-vibrate-0v84JX` (node). Consumer results
and published-type logs are retained under
`/private/tmp/solid-checker-structures-vibrate-consumer`. These are local
measurement artifacts. The regenerated compiled tier now supplies the same
result without any trial catalog or receipt-trust configuration: all three
correct-use consumers certify with zero findings, and each misuse reports one
SC1001 violation. Those results are retained as `*.embedded-tier.json` beside
the trial consumer results.

The integrated retained checkpoint measures 102 / 103 / 135 clean exports
for none / browser / node, against 101 / 101 / 131 previously, with the full
721-export surface retained and no formerly clean export lost. The gains are
`frequencyToPattern` on every host, `url:setLocationFallback` in browser, and
`sensors:createCompass`, `sensors:createGyroscope` and `url:updateLocation`
in node. The node sensor branches return fresh numeric data objects; their
browser getter objects remain outside this proof boundary.

Packages at the checkpoint remain 1/97. Criteria 1/2/3 remain 83/6/4;
135 exports are accounted for, including 33 accounted as published defects.
The entire post-tier misuse ledger remains 2/123 reporting correctly, with
zero TypeScript errors across all 246 misuse/correct-use examples. Its
outcomes match the pre-tier run. The wider misuse objective remains open.

Delivery contains 1,448 bundles and 1,967 objects, all authenticated by
`every_bundle_this_build_carries_authenticates`. All 837 primitives bundle
identities and all 89 primitives package names are retained. Compared with
the previous tier, 32 non-primitives identities are withheld: the host-free
Viviana `@tanstack/solid-start-client` graph fails on a missing mandatory
`@tanstack/router-core` veto recipe, and two router bindings cite receipts
the selected tier does not carry. This is an unresolved orchestration and
delivery limitation, not evidence of a package runtime defect. No failed
gate or uncarried receipt is treated as authority.

Full source `make verify` passes (856.76 s, no `FAILED during step`). Post-tier
Rust tests, formatting, workspace Clippy, coverage (142 projects/737 findings),
ownership (41 cases/465 rows, zero pending), the 120-fixture contract corpus
and conformance pass. Legacy conformance contains zero active receipt-issued
bundle cases; the separate authentication test above covers the actual
accepted tier. The fresh application-import sweep remains **1/1,850 certified**,
with primitives **0/292**, across 37 of 38 apps. `en-passant` remains unmeasured
because its npm installation requires a git dependency the measurement refuses
to fetch. Partial admitted sites fall from 265 to 21 and sites without an
admitted contract rise from 1,584 to 1,828. No certified site is lost, but the
non-primitives orchestration/admission gap above costs partial coverage and
remains unresolved. The run was not retried around the external installation
blocker. Actual primitives environments at rc.4 and rc.8 remain unaudited;
no rc.3/rc.9 authority is copied across them.

The pure conversion helper has no separate intrinsic misuse class. Its
caller-side untracked read therefore supplies no invented criterion-3 ledger
entry. A complete export is not a complete package: the sibling exports keep
their independent open claims. Saved containers, unsupported leaves and
unclassified completions stay fail-closed.
