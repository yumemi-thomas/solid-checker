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

## Version ownership at a glance

The catalogs overlap by defect concept, not necessarily by API spelling. A
shared `SCxxxx` code means the same class of bug exists in both versions; each
catalog still owns its own rule name, signature checks, message, and fix hint —
and, in one deliberate case, its own severity: `SC1004` is a **warning** as
[`v1/components-return-once`](v1/components-return-once.md) (matching upstream
eslint-plugin-solid's advisory level for existing 1.x codebases) and an
**error** as
[`component-returns-conditionally`](component-returns-conditionally.md), where
the 2.0 catalog has no adoption legacy to accommodate. A suppression comment
carried through a migration therefore lands on a stricter rule.

`SC2003` is another deliberate adoption-policy exception: the proven dropped
writes reported by [`v1/no-direct-mutation`](v1/no-direct-mutation.md) keep the
**warning** tier of upstream's `reactivity` rule. The shared
[`no-direct-mutation`](no-direct-mutation.md) rule keeps that tier as well, so
the same defect does not change severity merely because a project migrates
dialects. The warning is compatibility policy, not lower certainty.

For the eslint-plugin-solid 0.14.5 surface, every rule enabled by upstream's
base policy keeps that policy's severity. Three rules that upstream ships off
are deliberately available and enabled here: `v1/no-proxy-apis` is an
**error** because it enforces a target-runtime compatibility constraint;
`v1/prefer-classlist` and `v1/prefer-show` are **warnings** because they are
stylistic preferences. Projects that accept one of those tradeoffs can disable
that exact rule in the shared project configuration described below.

| Category | Solid 1.x catalog | Solid 2.0 catalog |
| --- | --- | --- |
| Shared concepts (20 rules) | `v1/` names and 1.x fixes (`Suspense`, `onMount`, single-function effects) | Unprefixed names and 2.0 fixes (`Loading`, `onSettled`, split effects) |
| Version-only concepts | 16 rules: `v1/no-async-tracked-scope` plus the retained SC8001–SC8017 ESLint-era surface | 15 rules: actions, `flush`, `resolve`, the 2.0-only leaf/directive restrictions, async computations and their SSR hydration options, the server surface (HTTP response head, server functions), and their proof obligations |
| Catalog size | 36 rules | 35 rules |

The analyzer beneath these catalogs is mostly shared. Version-specific
primitive names, callback behavior, owners, and boundaries come from the
selected dialect vocabulary; the catalog then decides whether the resulting
IR table has a rule and how that rule speaks to its user.

Findings come in two kinds:

- **violation** — the analyzer proved the code misbehaves at runtime.
- **uncertifiable** — the analyzer could not prove the code correct; the page for
  each `SC9xxx` rule explains how to make the code provable.

Uncertifiable findings normally carry **error** severity, including the ones
the owner rules (`SC4001`–`SC4004`) emit for exported functions whose callers
the analyzer cannot see (for `SC4004` this is no escalation: its proven form
is already an error, mirroring the runtime's dev-mode
`SETTLED_CLEANUP_UNOWNED` throw), and the ones `SC5001`
[pending-async-untracked-read](pending-async-untracked-read.md) emits when a
source's options argument cannot be read (an unreadable `loadingValue`
declaration would make the read safe during the first flight, so the throw is
no longer proven). The sole exception is `SC9011`
`reactive-source-uncaptured`: it has no proven-violation form and is
advisory-by-design, warning that an undescribed package boundary prevents the
analyzer from following a reactive source. A rule's severity in the manifest
describes its proven violation form when one exists.

## Rule configuration

Every rule in either catalog accepts an `enabled` boolean in the project-level
`.solid-checker/rule-options.json` document. Rules default to enabled. The
document is discovered by the same ancestor walk as
`.solid-checker/contracts/`, so the CLI, daemon, LSP, and ESLint snapshots all
apply the same policy.

Six of the 1.x ESLint-surface rules also carry the upstream options their
behaviour depends on: [v1/event-handlers](v1/event-handlers.md),
[v1/no-innerhtml](v1/no-innerhtml.md),
[v1/self-closing-comp](v1/self-closing-comp.md),
[v1/prefer-classlist](v1/prefer-classlist.md),
[v1/style-prop](v1/style-prop.md), and
[v1/no-unknown-namespaces](v1/no-unknown-namespaces.md):

```json
{
  "schemaVersion": 1,
  "rules": {
    "v1/no-proxy-apis": { "enabled": false },
    "v1/no-innerhtml": { "enabled": true, "allowStatic": false },
    "v1/style-prop": { "styleProps": ["style", "css"] }
  }
}
```

An absent file means every catalog rule is enabled with its normal defaults.
A file naming an unknown rule, an unknown option key, or a non-boolean
`enabled` value fails the analysis rather than silently changing policy.

**Retired rule identities are the one exception, and they are tolerated rather
than honoured.** A rule this checker publishes and later removes stays in a
permanent registry (`solid-facts-backend`'s `dialect::RETIRED_RULES`), so a
project that had disabled it keeps loading instead of failing on a name the
checker itself deleted. The rule is gone either way: no catalog declares it, so
the disable is a no-op — the entry tolerates a stale key, it does not keep the
rule available behind an option. `docs/precision-backlog.md` records why each
identity went. As of 2026-08 the retired set is `invalid-cleanup-return`,
`cleanup-return-unresolved`, `invalid-refresh-target`, `invalid-affects-target`,
`affects-keys-on-accessor`, `refresh-target-unresolved`,
`affects-target-unresolved`, and `v1/imports` — all eight removed as duplicates
of a TypeScript diagnostic. Rule
names are exact identities: for example, disabling `refresh-target-unresolved`
would not have disabled `affects-target-unresolved`, even though both findings
shared the portable code SC9003 (both were removed in 2026-08; the same holds
for any pair sharing a code). There is deliberately no separate per-ESLint-options
channel; the npm adapter runs one analysis per project. Each configurable
rule's page documents its additional options and defaults.

## Tracking & component semantics

| Code | Rule | Severity |
| --- | --- | --- |
| SC1001 | [strict-read-untracked](strict-read-untracked.md) | warning |
| SC1002 | [reactive-read-after-await](reactive-read-after-await.md) | error |
| SC1003 | [component-props-destructure](component-props-destructure.md) | error |
| SC1004 | [component-returns-conditionally](component-returns-conditionally.md) | error |
| SC1005 | [uncalled-accessor](uncalled-accessor.md) | warning |
| SC1006 | [untracked-derived-function](untracked-derived-function.md) | warning |
| SC1007 | [expected-function-got-expression](expected-function-got-expression.md) | warning |

## JSX correctness

| Code | Rule | Severity |
| --- | --- | --- |
| SC8018 | [prefer-component-syntax](prefer-component-syntax.md) | warning |
| SC8019 | [no-implicit-draggable](no-implicit-draggable.md) | error |
| SC8020 | [valid-jsx-nesting](valid-jsx-nesting.md) | error |

## Writes & actions

| Code | Rule | Severity |
| --- | --- | --- |
| SC2001 | [reactive-write-in-owned-scope](reactive-write-in-owned-scope.md) | error |
| SC2002 | [action-called-in-owned-scope](action-called-in-owned-scope.md) | error |
| SC2003 | [no-direct-mutation](no-direct-mutation.md) | warning |
| SC2004 | [resolve-in-reactive-scope](resolve-in-reactive-scope.md) | error |

## Leaf owners & cleanup

| Code | Rule | Severity |
| --- | --- | --- |
| SC3001 | [cleanup-in-forbidden-scope](cleanup-in-forbidden-scope.md) | error |
| SC3002 | [primitive-in-leaf-owner](primitive-in-leaf-owner.md) | error |
| SC3003 | [flush-in-forbidden-scope](flush-in-forbidden-scope.md) | error |

## Ownership

| Code | Rule | Severity |
| --- | --- | --- |
| SC4001 | [no-owner-effect](no-owner-effect.md) | warning |
| SC4002 | [no-owner-cleanup](no-owner-cleanup.md) | warning |
| SC4003 | [no-owner-boundary](no-owner-boundary.md) | warning |
| SC4004 | [no-owner-settled-cleanup](no-owner-settled-cleanup.md) | error |

## Async

| Code | Rule | Severity |
| --- | --- | --- |
| SC5001 | [pending-async-untracked-read](pending-async-untracked-read.md) | error |
| SC5002 | [pending-async-forbidden-scope](pending-async-forbidden-scope.md) | warning |
| SC5003 | [async-outside-loading-boundary](async-outside-loading-boundary.md) | warning |
| SC5005 | [ssr-client-source-outside-loading-boundary](ssr-client-source-outside-loading-boundary.md) | error |

## Directives

| Code | Rule | Severity |
| --- | --- | --- |
| SC6001 | [primitive-in-directive-application](primitive-in-directive-application.md) | warning |

## API shapes

| Code | Rule | Severity |
| --- | --- | --- |
| SC7001 | [missing-effect-function](missing-effect-function.md) | error |
| SC7002 | [sync-node-received-async](sync-node-received-async.md) | error |
| SC7005 | [http-response-after-flush](http-response-after-flush.md) | warning |
| SC7006 | [server-function-module-directive](server-function-module-directive.md) | error |
| SC7007 | [server-function-rich-argument](server-function-rich-argument.md) | error |

SC7005–SC7007 describe the 2.0 server surface (`@solidjs/web`'s HTTP response
head and core server functions) and exist only in the 2.0 catalog: Solid 1.x
has neither. SC7005 is a **warning** by design — the post-flush drop only
occurs when the boundary settles after the shell flush, so the static finding
is conditional rather than a proven-unconditional failure. If the analyzed
project cannot establish whether SSR exists, SC7005 is additionally marked
uncertifiable rather than treating a missing server import as proof of CSR.

## Uncertifiable (analysis limits)

| Code | Rule | Severity |
| --- | --- | --- |
| SC9001 | [package-contract-export-missing](package-contract-export-missing.md) | error |
| SC9004 | [execution-map-incomplete](execution-map-incomplete.md) | error |
| SC9006 | [package-contract-callback-missing](package-contract-callback-missing.md) | error |
| SC9005 | [package-contract-missing](package-contract-missing.md) | error |
| SC9011 | [reactive-source-uncaptured](reactive-source-uncaptured.md) | warning |
| SC9012 | [reactive-dispatch-unresolved](reactive-dispatch-unresolved.md) | warning |

Five of the fine-grained rules decomposed out of `eslint-plugin-solid`'s
`reactivity` (see [the migration table](#solidreactivity--the-fine-grained-rules))
describe defects that exist in both language versions — `uncalled-accessor`,
`untracked-derived-function`, `expected-function-got-expression`,
`no-direct-mutation`, and `reactive-source-uncaptured` — so the 2.0 catalog
carries them too, under the same codes. `no-async-tracked-scope` stays
1.x-only: Solid 2.0 models async computations as a feature, and its async
surface is owned by SC5001–SC5003 and SC5005 (SC5004 remains the 1.x rule's
code — it names a different defect concept, so the new 2.0-only SSR rule takes
the next free code in the family).

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
| SC8018 | [v1/prefer-component-syntax](v1/prefer-component-syntax.md) | warning |
| SC8019 | [v1/no-implicit-draggable](v1/no-implicit-draggable.md) | error |
| SC8020 | [v1/valid-jsx-nesting](v1/valid-jsx-nesting.md) | error |
| SC9001 | [v1/package-contract-export-missing](v1/package-contract-export-missing.md) | error |
| SC9004 | [v1/execution-map-incomplete](v1/execution-map-incomplete.md) | error |
| SC9006 | [v1/package-contract-callback-missing](v1/package-contract-callback-missing.md) | error |
| SC9005 | [v1/package-contract-missing](v1/package-contract-missing.md) | error |
| SC9011 | [v1/reactive-source-uncaptured](v1/reactive-source-uncaptured.md) | warning |
| SC9012 | [v1/reactive-dispatch-unresolved](v1/reactive-dispatch-unresolved.md) | warning |

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
- **One defect class, one rule.** Each rule owns a distinct defect, so a
  single defect is never reported under two names: an untracked read is
  `v1/strict-read-untracked` and never also `v1/uncalled-accessor`; a read
  after an `await` is `v1/reactive-read-after-await` and not also an untracked
  read. A region that contains several defects still collects a finding per
  defect — an `async` callback in a tracked scope is
  `v1/no-async-tracked-scope`, and a reactive read after its `await` is a
  separate `v1/reactive-read-after-await` finding. One declared exception:
  a reactive handler expression (`onClick={count()}`) keeps both
  `v1/expected-function-got-expression` and `v1/strict-read-untracked` in this
  catalog, pinned by the upstream parity ledger's rule-split entry; the 2.0
  catalog lets the handler rule own the expression and suppresses the
  strict-read duplicate.

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
