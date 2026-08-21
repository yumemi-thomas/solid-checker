# Rule catalog reduction migration

The 2026-08 catalog reduction leaves 18 Solid 1.x rules and 26 Solid 2.0
rules. It removes policy checks that could not certify a Solid runtime defect,
merges related diagnostics, and gives retained rules product vocabulary.

There are two compatibility channels:

- `.solid-checker/rule-options.json` aliases transfer an old key's explicit
  enable/disable to its successor.
- ESLint rule keys are separate. Only the six one-to-one renames retain a
  deprecated delegating key for one minor release. Merge keys and deleted keys
  are breaking removals.

Inline `eslint-disable` comments cannot be redirected. Grep for every old key
below and update or remove the directive.

## Aliased configuration keys — 19

Disabling any old merge member disables the complete successor family.

| Code | Old key | Current key |
| --- | --- | --- |
| SC4001 | `v1/no-owner-effect` | `v1/missing-owner` |
| SC4001 | `v1/no-owner-cleanup` | `v1/missing-owner` |
| SC4001 | `v1/no-owner-boundary` | `v1/missing-owner` |
| SC4001 | `no-owner-effect` | `missing-owner` |
| SC4001 | `no-owner-cleanup` | `missing-owner` |
| SC4001 | `no-owner-boundary` | `missing-owner` |
| SC4001 | `no-owner-settled-cleanup` | `missing-owner` |
| SC9005 | `v1/package-contract-export-missing` | `v1/package-contract-incomplete` |
| SC9005 | `v1/package-contract-missing` | `v1/package-contract-incomplete` |
| SC9005 | `v1/package-contract-callback-missing` | `v1/package-contract-incomplete` |
| SC9005 | `package-contract-export-missing` | `package-contract-incomplete` |
| SC9005 | `package-contract-missing` | `package-contract-incomplete` |
| SC9005 | `package-contract-callback-missing` | `package-contract-incomplete` |
| SC1003 | `component-props-destructure` | `no-destructure` |
| SC1004 | `component-returns-conditionally` | `components-return-once` |
| SC1007 | `expected-function-got-expression` | `reactive-handler-frozen` |
| SC1007 | `v1/expected-function-got-expression` | `v1/reactive-handler-frozen` |
| SC2004 | `resolve-in-reactive-scope` | `resolve-in-tracked-scope` |
| SC7002 | `sync-node-received-async` | `sync-computation-received-async` |

The final six rows are the one-to-one renames whose old explicit ESLint keys
temporarily delegate. The thirteen merge-family keys do not remain in
`plugin.rules`: forwarding an explicitly configured member would silently
widen its domain.

## Declared configuration breaks — six

These names are accepted in `rule-options.json` as retired no-ops, but their
disables intentionally do not transfer to the wider successor. Diagnostics a
project suppressed under the old member can therefore reappear.

| Old key | Successor | Code |
| --- | --- | --- |
| `cleanup-in-forbidden-scope` | `leaf-owner-forbidden-call` | SC3001 |
| `primitive-in-leaf-owner` | `leaf-owner-forbidden-call` | SC3001 |
| `flush-in-forbidden-scope` | `leaf-owner-forbidden-call` | SC3001 |
| `pending-async-untracked-read` | `pending-async-unsuspendable-read` | SC5001 |
| `pending-async-forbidden-scope` | `pending-async-unsuspendable-read` | SC5001 |
| `ssr-client-source-outside-loading-boundary` | `async-outside-loading-boundary` | SC5003 |

Explicit ESLint keys and inline disables for all six must be replaced manually.

## Deleted keys with no successor — 25

These configuration keys remain loadable as retired no-ops. Remove them from
project configuration and remove their ESLint rules or inline disables.

| Former code | Deleted key | Reason |
| --- | --- | --- |
| SC1006 | `v1/untracked-derived-function` | SC1001 follows the helper call chain |
| SC1006 | `untracked-derived-function` | SC1001 follows the helper call chain |
| SC3001 | `v1/cleanup-in-forbidden-scope` | `createReaction` owns its invalidation callback |
| SC3002 | `v1/primitive-in-leaf-owner` | `createReaction` owns and disposes created primitives |
| SC6001 | `v1/primitive-in-directive-application` | v1 directive application preserves the surrounding owner |
| SC8019 | `v1/no-implicit-draggable` | generic HTML attribute policy |
| SC8019 | `no-implicit-draggable` | generic HTML attribute policy |
| SC8007 | `v1/no-array-handlers` | v1 intentionally supports handler/data pairs |
| SC8010 | `v1/no-react-deps` | v1 intentionally accepts an array seed |
| SC8001 | `v1/event-handlers` | naming and readability policy |
| SC8011 | `v1/no-react-specific-props` | intrinsic uses are TypeScript-owned; component props pass through |
| SC8012 | `v1/no-unknown-namespaces` | intrinsic uses are TypeScript-owned; component props pass through |
| SC8008 | `v1/no-innerhtml` | injection policy removed; SC8003 keeps proven content competition |
| SC8017 | `v1/style-prop` | CSS policy or TypeScript-owned |
| SC5004 | `v1/no-async-tracked-scope` | async alone is not defective; SC1002 owns proven post-await reads |
| SC8004 | `v1/jsx-no-script-url` | generic injection policy |
| SC8006 | `v1/jsx-uses-vars` | semantic reference facts already model JSX use |
| SC8009 | `v1/no-proxy-apis` | target-runtime compatibility policy |
| SC8016 | `v1/self-closing-comp` | formatting policy |
| SC8018 | `v1/prefer-component-syntax` | naming policy over runtime-valid calls |
| SC8018 | `prefer-component-syntax` | naming policy over runtime-valid calls |
| SC9004 | `v1/execution-map-incomplete` | producer-integrity invariant, no longer a project diagnostic |
| SC9004 | `execution-map-incomplete` | producer-integrity invariant, no longer a project diagnostic |
| SC8020 | `v1/valid-jsx-nesting` | generic HTML parser conformance |
| SC8020 | `valid-jsx-nesting` | generic HTML parser conformance |

The permanent registry also contains eight identities removed in the earlier
TypeScript-redundancy audit; those were already retired before this reduction.
This change itself adds 31 retired identities and 19 aliases.

## Preference default change

As of the beta following this catalog reduction, every retained `prefer-*`
rule is enabled by default: `prefer-for` and `prefer-show` in both dialects,
plus Solid 1.x `prefer-classlist`. Native projects opt out with
`enabled: false` in `.solid-checker/rule-options.json`; ESLint projects set the
corresponding generated dialect rule to `off`. The `preferences` preset,
`--enable-rule`, and `preferences-v1` / `preferences-v2` configs remain
accepted as redundant compatibility interfaces.
