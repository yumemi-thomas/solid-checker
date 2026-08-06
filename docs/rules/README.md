# Rules

solid-checker certifies the reactive correctness of Solid projects. Every finding
carries a stable diagnostic code (`SCxxxx`), a rule name, a message describing what
went wrong, and a hint describing how to fix it. This index links to the full
documentation page for each rule.

There is one catalog per **dialect**. The Solid 2.0 catalog below is the default
and its rule names are unprefixed; the Solid 1.x catalog lives in
[`v1/`](v1/) and every one of its names carries the `v1/` namespace, so a
project can configure both side by side and a finding always names the dialect
that produced it. A rule that means the same thing in both dialects keeps its
`SCxxxx` code, so a suppression comment survives a 1.x → 2.0 migration.

The dialect is chosen per analysed project from the `solid-js` version the
project actually resolves, and `--dialect solid-v1` / `--dialect solid-v2`
overrides that.

Findings come in two kinds:

- **violation** — the analyzer proved the code misbehaves at runtime.
- **uncertifiable** — the analyzer could not prove the code correct; the page for
  each `SC9xxx` rule explains how to make the code provable.

## Tracking & component semantics

| Code | Rule | Severity |
| --- | --- | --- |
| SC1001 | [strict-read-untracked](strict-read-untracked.md) | warning |
| SC1002 | [reactive-read-after-await](reactive-read-after-await.md) | error |
| SC1003 | [component-props-destructure](component-props-destructure.md) | error |
| SC1004 | [component-returns-conditionally](component-returns-conditionally.md) | error |

## Writes & actions

| Code | Rule | Severity |
| --- | --- | --- |
| SC2001 | [reactive-write-in-owned-scope](reactive-write-in-owned-scope.md) | error |
| SC2002 | [action-called-in-owned-scope](action-called-in-owned-scope.md) | error |

## Leaf owners & cleanup

| Code | Rule | Severity |
| --- | --- | --- |
| SC3001 | [cleanup-in-forbidden-scope](cleanup-in-forbidden-scope.md) | error |
| SC3002 | [primitive-in-leaf-owner](primitive-in-leaf-owner.md) | error |
| SC3003 | [flush-in-forbidden-scope](flush-in-forbidden-scope.md) | error |
| SC3004 | [invalid-cleanup-return](invalid-cleanup-return.md) | error |
| SC3005 | [settled-cleanup-unowned](settled-cleanup-unowned.md) | error |

## Ownership

| Code | Rule | Severity |
| --- | --- | --- |
| SC4001 | [no-owner-effect](no-owner-effect.md) | warning |
| SC4002 | [no-owner-cleanup](no-owner-cleanup.md) | warning |
| SC4003 | [no-owner-boundary](no-owner-boundary.md) | warning |

## Async

| Code | Rule | Severity |
| --- | --- | --- |
| SC5001 | [pending-async-untracked-read](pending-async-untracked-read.md) | error |
| SC5002 | [pending-async-forbidden-scope](pending-async-forbidden-scope.md) | warning |
| SC5003 | [async-outside-loading-boundary](async-outside-loading-boundary.md) | warning |

## Directives

| Code | Rule | Severity |
| --- | --- | --- |
| SC6001 | [primitive-in-directive-application](primitive-in-directive-application.md) | error |

## API shapes

| Code | Rule | Severity |
| --- | --- | --- |
| SC7001 | [missing-effect-function](missing-effect-function.md) | error |
| SC7002 | [sync-node-received-async](sync-node-received-async.md) | error |
| SC7003 | [invalid-refresh-target](invalid-refresh-target.md) | error |
| SC7003 | [invalid-affects-target](invalid-affects-target.md) | error |
| SC7004 | [affects-keys-on-accessor](affects-keys-on-accessor.md) | error |

## Uncertifiable (analysis limits)

| Code | Rule | Severity |
| --- | --- | --- |
| SC9001 | [package-contract-export-missing](package-contract-export-missing.md) | error |
| SC9002 | [cleanup-return-unresolved](cleanup-return-unresolved.md) | error |
| SC9003 | [refresh-target-unresolved](refresh-target-unresolved.md) | error |
| SC9003 | [affects-target-unresolved](affects-target-unresolved.md) | error |
| SC9004 | [execution-map-incomplete](execution-map-incomplete.md) | error |
| SC9005 | [package-contract-missing](package-contract-missing.md) | error |


## Solid 1.x

The Solid 1.x catalog is documented in [`v1/`](v1/). It covers the same engine
analysis as the 2.0 catalog where 1.x has the concept, plus the whole
`eslint-plugin-solid` 0.14.5 rule surface, so a project migrating off that
plugin keeps its rule names under the `v1/` namespace.

| Code | Rule | Severity |
| --- | --- | --- |
| SC1001 | [v1/strict-read-untracked](v1/strict-read-untracked.md) | warning |
| SC1002 | [v1/reactive-read-after-await](v1/reactive-read-after-await.md) | error |
| SC1003 | [v1/no-destructure](v1/no-destructure.md) | error |
| SC1004 | [v1/components-return-once](v1/components-return-once.md) | warning |
| SC1005 | [v1/uncalled-accessor](v1/uncalled-accessor.md) | warning |
| SC1006 | [v1/untracked-derived-function](v1/untracked-derived-function.md) | warning |
| SC1007 | [v1/expected-function-got-expression](v1/expected-function-got-expression.md) | warning |
| SC2001 | [v1/reactive-write-in-owned-scope](v1/reactive-write-in-owned-scope.md) | error |
| SC2003 | [v1/no-direct-mutation](v1/no-direct-mutation.md) | warning |
| SC3001 | [v1/cleanup-in-forbidden-scope](v1/cleanup-in-forbidden-scope.md) | error |
| SC3002 | [v1/primitive-in-leaf-owner](v1/primitive-in-leaf-owner.md) | error |
| SC4001 | [v1/no-owner-effect](v1/no-owner-effect.md) | warning |
| SC4002 | [v1/no-owner-cleanup](v1/no-owner-cleanup.md) | warning |
| SC4003 | [v1/no-owner-boundary](v1/no-owner-boundary.md) | warning |
| SC5004 | [v1/no-async-tracked-scope](v1/no-async-tracked-scope.md) | warning |
| SC6001 | [v1/primitive-in-directive-application](v1/primitive-in-directive-application.md) | error |
| SC7001 | [v1/missing-effect-function](v1/missing-effect-function.md) | error |
| SC8001 | [v1/event-handlers](v1/event-handlers.md) | warning |
| SC8002 | [v1/imports](v1/imports.md) | warning |
| SC8003 | [v1/jsx-no-duplicate-props](v1/jsx-no-duplicate-props.md) | error |
| SC8004 | [v1/jsx-no-script-url](v1/jsx-no-script-url.md) | error |
| SC8005 | [v1/jsx-no-undef](v1/jsx-no-undef.md) | error |
| SC8006 | [v1/jsx-uses-vars](v1/jsx-uses-vars.md) | error |
| SC8007 | [v1/no-array-handlers](v1/no-array-handlers.md) | error |
| SC8008 | [v1/no-innerhtml](v1/no-innerhtml.md) | error |
| SC8009 | [v1/no-proxy-apis](v1/no-proxy-apis.md) | error |
| SC8010 | [v1/no-react-deps](v1/no-react-deps.md) | warning |
| SC8011 | [v1/no-react-specific-props](v1/no-react-specific-props.md) | warning |
| SC8012 | [v1/no-unknown-namespaces](v1/no-unknown-namespaces.md) | error |
| SC8013 | [v1/prefer-classlist](v1/prefer-classlist.md) | warning |
| SC8014 | [v1/prefer-for](v1/prefer-for.md) | error |
| SC8015 | [v1/prefer-show](v1/prefer-show.md) | warning |
| SC8016 | [v1/self-closing-comp](v1/self-closing-comp.md) | warning |
| SC8017 | [v1/style-prop](v1/style-prop.md) | warning |
| SC9001 | [v1/package-contract-export-missing](v1/package-contract-export-missing.md) | error |
| SC9004 | [v1/execution-map-incomplete](v1/execution-map-incomplete.md) | error |
| SC9005 | [v1/package-contract-missing](v1/package-contract-missing.md) | error |
| SC9011 | [v1/reactive-source-uncaptured](v1/reactive-source-uncaptured.md) | warning |

## Migrating from eslint-plugin-solid

Nineteen of `eslint-plugin-solid` 0.14.5's twenty rules map to one `v1/` rule
each, under the same name. The twentieth, `reactivity`, does not: it is one
rule reporting eight unrelated defects behind eight message ids, and reporting
them all under one name means a project cannot silence the one it disagrees
with without silencing the seven it wants. It is split.

### `solid/reactivity` → the fine-grained rules

| Upstream message id | What it reports | v1 rule |
| --- | --- | --- |
| `untrackedReactive` | a reactive value read where nothing tracks | [v1/strict-read-untracked](v1/strict-read-untracked.md) |
| `untrackedReactive`, after an `await` | a read in the continuation of an async computation, where tracking has already ended | [v1/reactive-read-after-await](v1/reactive-read-after-await.md) |
| `untrackedReactive`, on a props destructure | props unwrapped into frozen locals | [v1/no-destructure](v1/no-destructure.md) |
| `badSignal` | a proven accessor used uncalled in a value-only position: an untagged template interpolation, coercive operator, computed key, or native JSX value attribute | [v1/uncalled-accessor](v1/uncalled-accessor.md) |
| `badUnnamedDerivedSignal` | an anonymous function that closes over a reactive value and is neither tracked nor deferred by the position it sits in | [v1/untracked-derived-function](v1/untracked-derived-function.md) |
| `expectedFunctionGotExpression` | a reactive expression in a position whose contract is a function, so it is evaluated once instead of per read | [v1/expected-function-got-expression](v1/expected-function-got-expression.md) |
| `noWrite` | a signal reassigned, or a props/store member written through | [v1/no-direct-mutation](v1/no-direct-mutation.md) |
| `noAsyncTrackedScope` | an `async` function passed where a tracked computation is expected | [v1/no-async-tracked-scope](v1/no-async-tracked-scope.md) |
| `shouldDestructure`, `shouldAssign` | the result of `createSignal`/`createStore`/`createMemo` not captured in the shape the analyzer can follow — upstream's own analysis-integrity warnings | not ported: these warn about upstream's *analyzer* losing track, and this checker follows the value regardless; the three upstream cases are recorded as `evidence-backed` deviations in `fixtures/upstream-parity/deviations.json` |
| — (no upstream id) | a reactive source passed to a package-imported function nothing describes — the checker's own uncertifiable surface | [v1/reactive-source-uncaptured](v1/reactive-source-uncaptured.md) |

Two consequences worth knowing before switching a project over:

- **The split is not a filter.** Each rule runs the checker's own analysis —
  compiler execution facts, TypeScript type facts, package contracts — rather
  than upstream's syntactic pattern match, so the findings are not one-to-one
  with upstream's. Where the checker can prove more it reports more, and where
  upstream guessed it reports less. `v1/reactive-source-uncaptured` is the one
  rule that exists to say "the analyzer could not follow this", which is why it
  is the only `uncertifiable` member of the set.
- **One defect, one rule.** No defect is reported by two rules of the set. An
  untracked read is `v1/strict-read-untracked` and never also
  `v1/uncalled-accessor`; a read after an `await` is
  `v1/reactive-read-after-await` and not also an untracked read.

### The other nineteen

`components-return-once`, `event-handlers`, `imports`,
`jsx-no-duplicate-props`, `jsx-no-script-url`, `jsx-no-undef`, `jsx-uses-vars`,
`no-array-handlers`, `no-destructure`, `no-innerhtml`, `no-proxy-apis`,
`no-react-deps`, `no-react-specific-props`, `no-unknown-namespaces`,
`prefer-classlist`, `prefer-for`, `prefer-show`, `self-closing-comp` and
`style-prop` each keep their upstream name as `v1/<name>`.

`v1/jsx-uses-vars` is in the catalog and never fires: upstream's rule exists
only to mark JSX-referenced variables as used so `no-unused-vars` does not
report them, and TypeScript's own reference facts already model those uses. It
is listed so a config naming it resolves rather than erroring, and documented so
the silence is not mistaken for a gap.
