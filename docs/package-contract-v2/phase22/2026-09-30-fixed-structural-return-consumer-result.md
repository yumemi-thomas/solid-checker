# Published fixed-tuple return: certification and consumer result

Date: 2026-09-30. Scope: ADR 0172's first complete published export.
The overall primitives checkpoint and regenerated tier are still in progress.

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
measurement artifacts, not the delivered compiled tier.

The pure conversion helper has no separate intrinsic misuse class. Its
caller-side untracked read therefore supplies no invented criterion-3 ledger
entry. A complete export is not a complete package: the sibling exports keep
their independent open claims. Saved containers, unsupported leaves and
unclassified completions stay fail-closed.
