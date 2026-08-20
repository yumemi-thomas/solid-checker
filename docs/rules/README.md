# Rules

solid-checker has two dialect-owned catalogs: 18 rules for Solid 1.x and 26
for Solid 2.0. Solid 1.x findings keep a `v1/` prefix; Solid 2.0 findings are
unprefixed. The checker detects the installed `solid-js` major, with
`--dialect solid-v1` and `--dialect solid-v2` available for unusual layouts.

A shared `SCxxxx` code names one defect concept across dialects, while the rule
name is the configurable external identity. Findings are either:

- **violation** — semantic and execution facts prove a runtime defect;
- **uncertifiable** — required evidence is unavailable, so correctness cannot
  be certified.

The catalogs share 16 concepts. Solid 1.x adds `jsx-no-undef` and
`prefer-classlist`; Solid 2.0 adds ten rules for actions, tracked `resolve`, leaf
owners, directives, async computations, and the server surface.

## Configuration

Proof-backed and uncertifiable rules are enabled by default. The reactive-input
`prefer-for` and `prefer-show` preferences are also enabled by default and can
be opted out individually. Solid 1.x `prefer-classlist` remains opt-in through
the dialect-neutral `preferences` preset:

```sh
solid-checker --project tsconfig.json --preset preferences
solid-checker --project tsconfig.json --enable-rule v1/prefer-classlist
```

Both flags are repeatable. Project configuration lives in
`.solid-checker/rule-options.json`; an explicit `enabled: false` wins over CLI
enablement, and `enabled: true` enables a preference without a preset:

```json
{
  "schemaVersion": 1,
  "rules": {
    "v1/prefer-show": { "enabled": false },
    "v1/prefer-classlist": {
      "enabled": true,
      "classnames": ["cn", "clsx"]
    }
  }
}
```

ESLint exposes generated `v1` and `v2` default configs, which include
`prefer-for` and `prefer-show`, plus opt-in `preferences-v1` and
`preferences-v2` configs. The latter is empty for Solid 2.0; in Solid 1.x it
adds `prefer-classlist`. Setting either control-flow rule to `off` opts out.

Renamed and merged configuration keys, retired identities, and the six merges
whose disables deliberately do not transfer are listed in the
[catalog migration note](../rule-catalog-migration.md).

## Solid 2.0 catalog — 26 rules

| Code | Rule | Severity | Default |
| --- | --- | --- | --- |
| SC1001 | [strict-read-untracked](strict-read-untracked.md) | warning | on |
| SC1002 | [reactive-read-after-await](reactive-read-after-await.md) | error | on |
| SC1003 | [no-destructure](no-destructure.md) | error | on |
| SC1004 | [components-return-once](components-return-once.md) | error | on |
| SC1005 | [uncalled-accessor](uncalled-accessor.md) | warning | on |
| SC1007 | [reactive-handler-frozen](reactive-handler-frozen.md) | warning | on |
| SC2001 | [reactive-write-in-owned-scope](reactive-write-in-owned-scope.md) | error | on |
| SC2002 | [action-called-in-owned-scope](action-called-in-owned-scope.md) | error | on |
| SC2003 | [no-direct-mutation](no-direct-mutation.md) | warning | on |
| SC2004 | [resolve-in-tracked-scope](resolve-in-tracked-scope.md) | error | on |
| SC3001 | [leaf-owner-forbidden-call](leaf-owner-forbidden-call.md) | error | on |
| SC4001 | [missing-owner](missing-owner.md) | warning | on |
| SC5001 | [pending-async-unsuspendable-read](pending-async-unsuspendable-read.md) | error | on |
| SC5003 | [async-outside-loading-boundary](async-outside-loading-boundary.md) | warning | on |
| SC6001 | [primitive-in-directive-application](primitive-in-directive-application.md) | warning | on |
| SC7001 | [missing-effect-function](missing-effect-function.md) | error | on |
| SC7002 | [sync-computation-received-async](sync-computation-received-async.md) | error | on |
| SC7005 | [http-response-after-flush](http-response-after-flush.md) | warning | on |
| SC7006 | [server-function-module-directive](server-function-module-directive.md) | error | on |
| SC7007 | [server-function-rich-argument](server-function-rich-argument.md) | error | on |
| SC8003 | [jsx-no-duplicate-props](jsx-no-duplicate-props.md) | error | on |
| SC8014 | [prefer-for](prefer-for.md) | error | on |
| SC8015 | [prefer-show](prefer-show.md) | warning | on |
| SC9005 | [package-contract-incomplete](package-contract-incomplete.md) | error | on |
| SC9011 | [reactive-source-uncaptured](reactive-source-uncaptured.md) | warning | on |
| SC9012 | [reactive-dispatch-unresolved](reactive-dispatch-unresolved.md) | warning | on |

`http-response-after-flush`, `package-contract-incomplete`,
`reactive-source-uncaptured`, and `reactive-dispatch-unresolved` can produce
uncertifiable results. Their pages name the missing evidence and remediation.

## Solid 1.x catalog — 18 rules

| Code | Rule | Severity | Default |
| --- | --- | --- | --- |
| SC1001 | [v1/strict-read-untracked](v1/strict-read-untracked.md) | warning | on |
| SC1002 | [v1/reactive-read-after-await](v1/reactive-read-after-await.md) | error | on |
| SC1003 | [v1/no-destructure](v1/no-destructure.md) | error | on |
| SC1004 | [v1/components-return-once](v1/components-return-once.md) | warning | on |
| SC1005 | [v1/uncalled-accessor](v1/uncalled-accessor.md) | warning | on |
| SC1007 | [v1/reactive-handler-frozen](v1/reactive-handler-frozen.md) | warning | on |
| SC2001 | [v1/reactive-write-in-owned-scope](v1/reactive-write-in-owned-scope.md) | error | on |
| SC2003 | [v1/no-direct-mutation](v1/no-direct-mutation.md) | warning | on |
| SC4001 | [v1/missing-owner](v1/missing-owner.md) | warning | on |
| SC7001 | [v1/missing-effect-function](v1/missing-effect-function.md) | error | on |
| SC8003 | [v1/jsx-no-duplicate-props](v1/jsx-no-duplicate-props.md) | error | on |
| SC8005 | [v1/jsx-no-undef](v1/jsx-no-undef.md) | error | on |
| SC8013 | [v1/prefer-classlist](v1/prefer-classlist.md) | warning | preferences |
| SC8014 | [v1/prefer-for](v1/prefer-for.md) | error | on |
| SC8015 | [v1/prefer-show](v1/prefer-show.md) | warning | on |
| SC9005 | [v1/package-contract-incomplete](v1/package-contract-incomplete.md) | error | on |
| SC9011 | [v1/reactive-source-uncaptured](v1/reactive-source-uncaptured.md) | warning | on |
| SC9012 | [v1/reactive-dispatch-unresolved](v1/reactive-dispatch-unresolved.md) | warning | on |

## Migrating from eslint-plugin-solid

The retained semantic surface maps to `v1/no-destructure`,
`v1/components-return-once`, `v1/jsx-no-duplicate-props`, `v1/jsx-no-undef`,
and the three retained preference rules. Upstream's broad `reactivity` rule is
split among SC1001, SC1002, SC1003, SC1005, SC1007, SC2003, SC9011, and SC9012
so one configuration key never suppresses unrelated defect classes.

Formatting, browser-policy, CSS-policy, compatibility-policy, and analyzer
bookkeeping rules were retired instead of being presented as certified runtime
defects. See the [migration note](../rule-catalog-migration.md) for every old
key, successor, code, and deliberate break.
