# Rules

solid-checker ships one dialect-owned catalog: 27 rules for Solid 2.0, with
unprefixed rule names. The checker detects the installed `solid-js` major and
**refuses** a project whose runtime it has no vocabulary for
([`unsupported-solid-runtime`](unsupported-solid-runtime.md)) rather than
analyzing it under the wrong language; `--dialect solid-v2` is available for
unusual layouts. The Solid 1.x catalog was retired on 2026-09-16 — see
[ADR 0110](../adr/0110-the-checker-analyzes-solid-2-only.md), and
[the catalog migration note](../rule-catalog-migration.md) for mapping a `v1/`
suppression onto its 2.0 identity.

An `SCxxxx` code names one defect concept and the rule name is the configurable
external identity; the two are separate so a code can outlive a rename and a
future second catalog can share a concept without sharing a name. Findings are either:

- **violation** — semantic and execution facts prove a runtime defect;
- **uncertifiable** — required evidence is unavailable, so correctness cannot
  be certified.

The catalogs share 16 concepts. Solid 1.x adds `jsx-no-undef` and
`prefer-classlist`; Solid 2.0 adds ten rules for actions, tracked `resolve`, leaf
owners, directives, async computations, and the server surface.

## Configuration

Every catalog rule is enabled by default, including `prefer-for`,
`prefer-show`, and Solid 1.x `prefer-classlist`. Native projects opt out in
`.solid-checker/rule-options.json`:

```json
{
  "schemaVersion": 1,
  "rules": {
    "prefer-show": { "enabled": false },
    "v1/prefer-classlist": { "enabled": false }
  }
}
```

An explicit `enabled: false` wins over defaults, presets, and CLI enablement.
The repeatable `--preset preferences` and `--enable-rule` interfaces remain
accepted for compatibility, but are redundant for these rules. Other
per-rule options live in the same project file:

```json
{
  "schemaVersion": 1,
  "rules": {
    "v1/prefer-classlist": {
      "classnames": ["cn", "clsx"]
    }
  }
}
```

ESLint's generated `v1` and `v2` configs include every `prefer-*` rule.
Setting one to `off` opts out. The legacy `preferences-v1` and
`preferences-v2` configs remain available for compatibility and are now
redundant.

Renamed and merged configuration keys, retired identities, and the six merges
whose disables deliberately do not transfer are listed in the
[catalog migration note](../rule-catalog-migration.md).

## Solid 2.0 catalog — 27 rules

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
| SC9013 | [unsupported-solid-runtime](unsupported-solid-runtime.md) | error | on |

`http-response-after-flush`, `package-contract-incomplete`,
`reactive-source-uncaptured`, `reactive-dispatch-unresolved`, and
`unsupported-solid-runtime` can produce uncertifiable results. Their pages name
the missing evidence and remediation.

`unsupported-solid-runtime` is the one identity here the rules engine never
produces: dialect detection emits it before analysis, and it appears **alone**
when it appears at all. See ADR 0110.

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
