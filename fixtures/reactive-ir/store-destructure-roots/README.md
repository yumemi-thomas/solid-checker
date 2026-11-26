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

**Part 2 (facts schema 110).** A plain assignment's object target (`({ v } = s)`, `({ v } = merge(s, {…}))`) and an object pattern that is
an element of a declarator's array pattern over a literal (`const [{ v }] = [s]`, `const [, { v }] = [0, s]`: every literal element up to
it a plain value, array iteration intact) are `destructure_sites`, which the same readers take as declarators.

| component | outcome | Chrome |
|---|---|---|
| `AssignBody`, `AssignMergeBody`, `AssignMember` (`{ v: o.x }`), `ArrayBody`, `ArraySecond` | SC1003 a violation (main: uncertifiable) | STRICT_READ_UNTRACKED |
| `AssignMemo`, `ArrayMemo` | not claimed (main: uncertifiable census rows) | silent |
| `ArrayHole` (a hole before the element), `ArraySpread` (a spread literal), `AssignProps` (a props root) | uncertifiable, as before | `ArrayHole`, `ArraySpread`: STRICT_READ_UNTRACKED; `AssignProps`: silent |

Chrome: `fixtures/runtime-oracle/store-destructure-roots`. Stubs: `absent-guard-prop`'s `solid-js.d.ts`.

**ADR 1226.** `ContextMerge` (`merge(ctx.s1, { w: 1 })` destructured, the store from a context) and `HolderMerge` (`merge(holder.s1, …)`): no rule reads the slots, since `destructure_root` names a plain reference only, so the value-use census keeps an SC9012 `value-in-carrier` row there (uncertifiable; main: a certified clean for the context form, Chrome STRICT_READ_UNTRACKED).

**ADR 1228.** `ContextForOf` (`for (const { v } of [ctx.s1])`) and `PropAssign` (`({ v } = props.s1)`): the same census row at the pattern in the body, none in the memo's compute.
