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
counts a case as matching when the rule under test fired for an `invalid` case
and stayed silent for a `valid` one.

**389 of 465 match.** The other 76 are declared in `deviations.json`, one entry
per case, each with a status and a reason:

| status | count | meaning |
| --- | --- | --- |
| `deliberate` | 37 | the checker differs on purpose, and the reason says why |
| `gap` | 39 | upstream reports something the checker does not, and closing it is outstanding work |

The comparison is exact in both directions. A case that starts deviating fails,
and so does one that stops, so an inherited false positive cannot arrive
quietly and a fix cannot silently rot. `make parity-update` rewrites the file,
keeping existing reasons and marking anything new `triage`, which the
comparison rejects — a new deviation has to be explained, not merely recorded.

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

## Fields

| field | meaning |
| --- | --- |
| `code` | the case source, verbatim |
| `errors` | how many diagnostics upstream expects (0 for valid cases) |
| `messageIds` | upstream's message ids, which the reactivity mapping keys on |
| `options` | upstream rule options, or `null` — the checker's rules are not configurable, so a case carrying options can only be a deviation |
| `output` | upstream's autofix result, or `null` |
| `tsOnly` | upstream runs the case only under a TypeScript parser |
