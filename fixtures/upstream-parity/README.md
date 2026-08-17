# Upstream parity corpus

Every `valid` and `invalid` case from the rule test suites of
[eslint-plugin-solid](https://github.com/solidjs-community/eslint-plugin-solid)
0.14.5, extracted as data: **465 cases across 19 rules**, 236 valid and 229
invalid.

Extracted from upstream commit `6d3bc311` (2025-11-18) by
`scripts/extract-upstream-cases.mjs`, which stubs the three imports each test
file makes and evaluates the module — the files are uniformly
`export const cases = run(name, rule, { valid, invalid })`, so the cases come
out as data rather than being transcribed by hand. Re-run it against a newer
upstream to refresh:

```bash
node scripts/extract-upstream-cases.mjs <upstream>/packages/eslint-plugin-solid/test/rules fixtures/upstream-parity
```

## What is not here

`jsx-uses-vars` is upstream's twentieth suite and is absent. Its tests import
`eslint-v8` to drive ESLint's own `no-unused-vars` and assert that JSX
references mark a variable used, so the cases exercise ESLint's core rule
rather than upstream's. The checker has no equivalent to assert against —
TypeScript reference facts already model those uses — so the rule is
catalogued and deliberately never fires. See `docs/rules/v1/jsx-uses-vars.md`.

## How it is consumed

```bash
make parity
```

`scripts/parity.mjs` materialises every case as its own file in one synthetic
project (`harness.json` holds the scaffolding), runs the checker once, and
compares three things against what upstream's own test asserts:

| dimension | what is compared | against |
| --- | --- | --- |
| `fired` | the rule under test spoke for an `invalid` case, stayed silent for a `valid` one | the case's kind |
| `counts` | how many diagnostics the rule produced | the case's `errors` |
| `outputs` | what applying the rule's fixes to the case source produces | the case's `output` |

Counts and outputs are only asked of a case that already agrees on `fired`: a
case where the rule stayed silent has no count or fix to compare, and recording
0-against-2 there would be the same disagreement written down twice. That is
what accounts for the 202 of 229 invalid cases below — the other 27 are the
invalid half of the `fired` ledger.

```
429/465 upstream cases match; 36 deviate
202/229 invalid cases compared on diagnostic count; 3 deviate
59 of those compared on fix output; 16 deviate (46 upstream autofixes the checker offers no fix for)
all 55 deviations declared: fired 23 evidence-backed/11 fact-unavailable/2 policy; counts 2 per-site/1 rule-split; outputs 4 cosmetic/6 different-strategy/6 tighter-cleanup
```

### What is compared, exactly

- **Counts** are per rule, not per message. Upstream's `reactivity` is one
  rule, so every checker rule it maps onto counts towards the same total — a
  case where the checker splits one upstream report into two rules is a
  deviation, not a free pass. All 229 invalid cases declare `errors`
  explicitly, so nothing is compared against an inferred number.
- **Outputs** are compared byte for byte. **No whitespace is normalised**, and
  no trailing newline is forgiven. What *is* undone is the harness's own two
  additions ("Two adjustments the harness makes", below): the Solid imports it prepends and the `export {};` plus
  final newline it appends are stripped, and only if both survived the fixes
  verbatim — a fix that rewrote across either affix leaves nothing sound to
  compare and is reported as uncompared rather than counted as agreement.
- Fixes are applied in **one pass**, the way ESLint's own fixer does: edits in
  source order, and a fix overlapping text an earlier fix already rewrote is
  skipped. A fix is all-or-nothing, since applying half of a multi-edit fix
  would emit text no fix ever proposed. A finding offering several
  *alternative* fixes is not comparable at all — the harness has no basis for
  choosing between them — and is reported as uncompared. No case currently
  hits either path.

### What is still not compared

- **Fix coverage.** In 46 of the 105 cases that reach this dimension, upstream
  declares an `output` and the checker reports the defect but offers no fix:
  all 28 `no-destructure`, all 6 `prefer-classlist`, all 5 `prefer-for`, 3
  `event-handlers`, and one each of `components-return-once`, `imports`,
  `jsx-no-undef`, and `style-prop`. An absent fix is a coverage gap, not a
  disagreement about what the right fix would be, so the count is printed but
  not ratcheted — a rule that grows a fix starts being compared on it, and only
  a *disagreeing* fix fails.
- **Message text, ids, and locations.** The checker's messages and diagnostic
  ids are its own; upstream's `messageIds` are used only to key the reactivity
  rule mapping.

### Two adjustments the harness makes, and why

- **Solid imports are supplied.** Upstream's cases call `createEffect` and
  friends without importing them, because its rules match on the name. The
  checker resolves a primitive through its import instead — stricter, and the
  reason a local function called `createEffect` is not mistaken for Solid's —
  so an unimported case would test resolution rather than the rule. The import
  upstream assumes is added, except for `jsx-no-undef`, whose cases are about
  names that are *not* defined.
- **Each case is a module.** A file with no import or export is a *script* to
  TypeScript, so its top-level names would share one global scope and a
  `Component` declared by one case would satisfy another's undefined
  reference. `export {}` restores the per-file scope upstream lints in.

## The deviation ledgers

Deviations are declared in `deviations.json`, one ledger per dimension, one
entry per case, each with a status and a reason. `counts` entries carry the two
numbers they describe and `outputs` entries carry the text the checker
produced, so a deviation that merely changes *magnitude* — 2-against-1 becoming
3-against-1, or a fix emitting different text — has to be re-explained instead
of coasting on a declaration it outgrew.

### `fired` — 88 entries

| status | count | meaning |
| --- | --- | --- |
| `evidence-backed` | 19 | compiler, runtime, type, or contract evidence makes the checker's result more precise than upstream's syntax heuristic |
| `fact-unavailable` | 12 | the isolated case supplies no fact that proves the relevant Solid or user-code contract, so the checker refuses to guess from a spelling convention |
| `typescript-owned` | 45 | the defect is already a TypeScript diagnostic on this exact code, so AGENTS.md's absolute rule puts it out of scope |
| `policy` | 12 | the checker intentionally enforces a stricter policy, or declines a name-only allowlist upstream carries |

`typescript-owned` is the only status whose claim is **mechanically verified**.
`scripts/parity-tsc-ownership.mjs` compiles the same bytes this harness lints
against the real published `solid-js` typings and fails when such a deviation sits
on a case TypeScript does not report *in the case's own code* — the harness's
prepended imports do not count, and neither do the incidental implicit-any and
cannot-find-name errors an untyped corpus is full of.

It exists precisely because `policy` cannot carry that claim. `policy` covers any
deliberate difference, including ones that say nothing about TypeScript, so
demanding a diagnostic for every `policy` entry asked for evidence of a claim
those entries never made. Splitting the two was the fix.

There used to be a fourth status, `unsupported-option`, for cases enabling a
non-default upstream option. It emptied when the six option-bearing rules
grew the upstream options surface (`.solid-checker/rule-options.json` —
"Rule options" in `CONTEXT.md`), and the harness now materialises those
cases with the case's own options.

These statuses distinguish improvements from scope choices. An
`evidence-backed` deviation is a result we actively want. A
`fact-unavailable` deviation is conservative: it prevents a false
inference from an unresolved spelling, but can either withhold a diagnostic or
retain a warning that a real type or package contract would settle. It may
therefore identify useful future fact coverage when the same pattern occurs in
real typed code. `policy` requires an explicit project decision, and
`typescript-owned` requires a diagnostic that a gate re-checks on every run.

There are no known correctness gaps hidden in this ledger — and that sentence is
now worth more than it was, because the same script also asserts the converse: no
finding may share a *span* with a TypeScript diagnostic unless the difference in
the two claims is written down. Thirty such overlaps are declared distinct (mostly
artefacts of upstream's untyped fragments inventing attribute names); four are
confirmed duplicates awaiting a narrowing, listed in
`docs/precision-backlog.md`.

### `counts` — 3 entries

| status | count | meaning |
| --- | --- | --- |
| `per-site` | 2 | the checker anchors one finding on each proven defect site where upstream reports the enclosing construct once |
| `rule-split` | 1 | one upstream `reactivity` message id maps onto several checker rules and more than one of them fires |

Both `per-site` entries are chains: a nested ternary whose two conditions are
each independently reactive (`components-return-once__invalid__06`), and two
nested derived functions each of which is independently unsubscribed
(`reactivity__invalid__10`). The `rule-split` entry is an async effect, where
upstream's one `noAsyncTrackedScope` becomes the checker's
`v1/no-async-tracked-scope` (the scope) plus `v1/reactive-read-after-await`
(the specific post-await read).

No case in either direction reports *fewer* diagnostics than upstream expects.

### `outputs` — 16 entries

| status | count | meaning |
| --- | --- | --- |
| `tighter-cleanup` | 6 | the applied text differs only because the checker's edit also removes the separator or whitespace upstream's leaves stranded |
| `different-strategy` | 6 | the checker repairs the defect with a different, also correct, edit |
| `cosmetic` | 4 | the same program; the difference is where a new statement lands or the order of names in a list |

`tighter-cleanup` is the five `no-react-deps` cases, where the checker emits
`createEffect(fn)` and upstream leaves `createEffect(fn, )`, plus
`no-react-specific-props__invalid__09`, where deleting `key` leaves `<div />`
rather than `<div  />`. `different-strategy` is every `imports` case that
carries a fix: upstream relocates the specifier into a declaration of the right
module, the checker rewrites the wrong module specifier in place. Two of those
are places the checker's text is the better one:
`imports__invalid__02` keeps the `type` modifier upstream's merge drops, and
`imports__invalid__05` fixes both wrong declarations in one pass where
ESLint's single pass skips the second, leaving upstream's recorded `output`
still importing `render` from `"solid-js"`. `cosmetic` is the four
`jsx-no-undef` auto-import cases: the checker inserts a new declaration at the
top of the file and sorts the names inside it, upstream inserts before the
first existing statement in first-appearance order.

### The ratchet

The comparison is exact in every direction, for every dimension. A case that
starts deviating fails, one that stops fails, and one whose numbers or fix text
merely change fails too — so an inherited false positive cannot arrive quietly,
a fix cannot silently rot, and an improvement cannot land without someone
looking at it. `make parity-update` rewrites the file, keeping existing reasons
and marking anything new — or anything whose recorded values moved — `triage`,
which the comparison rejects. A deviation has to be explained, not merely
recorded.

## Fields

| field | meaning |
| --- | --- |
| `code` | the case source, verbatim |
| `errors` | how many diagnostics upstream expects (0 for valid cases) |
| `messageIds` | upstream's message ids, which the reactivity mapping keys on |
| `options` | upstream rule options, or `null` — for the six rules carrying upstream's options (`.solid-checker/rule-options.json`), the harness materialises the case in its own project with exactly these options |
| `output` | upstream's autofix result, or `null` |
| `tsOnly` | upstream runs the case only under a TypeScript parser |
