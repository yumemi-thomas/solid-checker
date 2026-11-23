# store-destructure-roots

**Claim (ADR 1193).** A store destructure whose root is no plain identifier initializer reads every key from the store exactly when
`static_rules::destructure_root` names its root: the sole element of the array literal a `for…of` iterates (`for (const { v } of [s])`,
array iteration intact, no `for await`, one declarator), or the first source of `merge(s, {…})` when every later source is a closed
literal with none of the pattern's keys. SC1003 then claims it in setup as for `const { v } = s`, and elsewhere SC1001 places each key's
Get, except where the read's own role proves a fresh read, which keeps the earlier silence.

| component | outcome | Chrome |
|---|---|---|
| `ForOfBody`, `MergeBody` | SC1003 a violation (main: uncertifiable `destructure-root-unproven`) | STRICT_READ_UNTRACKED |
| `ForOfMemo`, `MergeMemo` (a memo's compute) | not claimed | silent |
| `ForOfTwo` (two elements), `MergeShadowed` (a later literal has the key), `MergeOpen` (a later source no literal) | uncertifiable, as before | `ForOfTwo`, `MergeOpen`: STRICT_READ_UNTRACKED; `MergeShadowed`: silent |

Chrome: `fixtures/runtime-oracle/store-destructure-roots`. Stubs: `absent-guard-prop`'s `solid-js.d.ts`.
