# Rules

solid-checker ships one dialect-owned catalog: 34 identities for Solid 2.0,
with unprefixed rule names: 31 rules the engine produces and three notices.
The checker detects the installed `solid-js` major and **refuses** a project
whose runtime it has no vocabulary for
([`unsupported-solid-runtime`](unsupported-solid-runtime.md)) rather than
analyzing it under the wrong language; `--dialect solid-v2` is available for
unusual layouts. A Solid 2 installation the vocabulary was not audited on is
analyzed with a notice
([`unaudited-solid-release`](unaudited-solid-release.md)). The Solid 1.x catalog was retired on 2026-09-16 — see
[ADR 0110](../decisions.md#adr-0110); a `v1/` name in a configuration is a
retired no-op (`RETIRED_RULES` in `rust/crates/solid-facts-backend/src/dialect.rs`).

An `SCxxxx` code names one defect concept and the rule name is the configurable
external identity; the two are separate so a code can outlive a rename and a
future second catalog can share a concept without sharing a name. Findings are either:

- **violation** — semantic and execution facts prove a runtime defect;
- **uncertifiable** — required evidence is unavailable, so correctness cannot
  be certified.

Most concepts were also carried by the retired 1.x catalog; the 2.0-only ones
cover actions, tracked `resolve` and `until`, leaf owners, directives, async
computations, and the server surface. `jsx-no-undef` and `prefer-classlist` had
no 2.0 counterpart and were dropped rather than re-homed (ADR 0110 § 2).

## Configuration

Every catalog rule is enabled by default, including `prefer-for` and
`prefer-show`. Native projects opt out in
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
accepted for compatibility, but are redundant for these rules. The one
per-rule option beyond `enabled` lives in the same project file: the
[valid-use-server](valid-use-server.md) `clientWrappers` list (upstream's
grammar: an exact name, `*` over `[a-zA-Z0-9_$]`, or an unanchored `/regex/`),
which names callee spellings the project itself declares to be client-side
wrappers. Any other key is refused.

```json
{
  "schemaVersion": 1,
  "rules": {
    "valid-use-server": {
      "clientWrappers": ["defineLoader", "define*", "/^use[A-Z]/"]
    }
  }
}
```

ESLint's generated `v1` and `v2` configs include every `prefer-*` rule.
Setting one to `off` opts out. The legacy `preferences-v1` and
`preferences-v2` configs remain available for compatibility and are now
redundant.

Renamed configuration keys transfer their setting through `RULE_ALIASES`, and
retired identities load as no-ops through `RETIRED_RULES`, both in
`rust/crates/solid-facts-backend/src/dialect.rs` ([ADR 0391](../decisions.md#adr-0391)).

## Solid 2.0 catalog — 34 rules

| Code | Rule | Severity | Default |
| --- | --- | --- | --- |
| SC1001 | [strict-read-untracked](strict-read-untracked.md) | warning | on |
| SC1002 | [reactive-read-after-await](reactive-read-after-await.md) | error | on |
| SC1003 | [no-destructure](no-destructure.md) | error | on |
| SC1004 | [components-return-once](components-return-once.md) | error | on |
| SC1005 | [uncalled-accessor](uncalled-accessor.md) | warning | on |
| SC1007 | [reactive-handler-frozen](reactive-handler-frozen.md) | warning | on |
| SC2001 | [no-write-in-pure-computation](no-write-in-pure-computation.md) | error | on |
| SC2002 | [action-called-in-owned-scope](action-called-in-owned-scope.md) | error | on |
| SC2003 | [no-store-mutation-outside-setter](no-store-mutation-outside-setter.md) | warning | on |
| SC2004, SC2005 | [no-await-primitive-in-tracked-scope](no-await-primitive-in-tracked-scope.md) | error | on |
| SC2006 | [flush-in-action](flush-in-action.md) | error | on |
| SC2007 | [static-dynamic-async-source](static-dynamic-async-source.md) | error | on |
| SC2008 | [flush-in-effect-callback](flush-in-effect-callback.md) | warning | on |
| SC3001 | [leaf-owner-forbidden-call](leaf-owner-forbidden-call.md) | error | on |
| SC4001 | [missing-owner](missing-owner.md) | warning | on |
| SC4002 | [missing-context-provider](missing-context-provider.md) | error | on |
| SC5001 | [pending-async-unsuspendable-read](pending-async-unsuspendable-read.md) | error | on |
| SC5003 | [async-outside-loading-boundary](async-outside-loading-boundary.md) | warning | on |
| SC6001 | [primitive-in-directive-application](primitive-in-directive-application.md) | warning | on |
| SC7001 | [missing-effect-function](missing-effect-function.md) | error | on |
| SC7002 | [sync-computation-received-async](sync-computation-received-async.md) | error | on |
| SC7005 | [http-response-after-flush](http-response-after-flush.md) | warning | on |
| SC7006 | [valid-use-server](valid-use-server.md) | error | on |
| SC7007 | [server-function-rich-argument](server-function-rich-argument.md) | error | on |
| SC8003 | [jsx-no-duplicate-props](jsx-no-duplicate-props.md) | error | on |
| SC8014 | [prefer-for](prefer-for.md) | error | on |
| SC8015 | [prefer-show](prefer-show.md) | warning | on |
| SC9005 | [package-behavior-unavailable](package-behavior-unavailable.md) | error | on |
| SC9011 | [reactive-source-uncaptured](reactive-source-uncaptured.md) | warning | on |
| SC9012 | [reactive-dispatch-unresolved](reactive-dispatch-unresolved.md) | warning | on |
| SC9013 | [unsupported-solid-runtime](unsupported-solid-runtime.md) | error | on |
| SC9014 | [unaudited-solid-release](unaudited-solid-release.md) | warning | on |
| SC9015 | [compiler-refused-source](compiler-refused-source.md) | error (warning notice for an unproven refusal) | on |

`http-response-after-flush`, `package-behavior-unavailable`,
`reactive-source-uncaptured`, `reactive-dispatch-unresolved`,
`unsupported-solid-runtime`, `unaudited-solid-release`, and
`compiler-refused-source` (for a refusal it does not prove, ADR 0399) can
produce uncertifiable results. Their pages name
the missing evidence and remediation.

SC9005 was named `package-contract-incomplete`; that name remains an alias of
`package-behavior-unavailable`, which means required package behavior is
unavailable from source. Package contracts and receipts are no longer consumed;
see [ADR 0312](../decisions.md#adr-0312).

`unsupported-solid-runtime`, `unaudited-solid-release` and
`compiler-refused-source` are the three identities here the rules engine never
produces. Dialect detection decides the first two, fact building the third.
The refusal replaces the analysis and appears **alone** when it appears at all
(ADR 0110). The notices accompany the analysis.

## Migrating from eslint-plugin-solid

A rule that maps onto an eslint-plugin-solid rule takes the upstream name
unless that name no longer describes what the checker proves, and its scope is
a superset of upstream's ([ADR 0381](../decisions.md#adr-0381)). Shipped renames (each old name is an alias, ADR 0391; the two names of a merged rule are limited to their own code, ADR 0384): SC2001
`no-write-in-pure-computation` (ADR 0382), SC2003
`no-store-mutation-outside-setter` (ADR 0383; upstream's array-method and
projection shapes are built, its `delete` is not: TS2790 on a required property), SC2004 and SC2005 together as
`no-await-primitive-in-tracked-scope` (ADR 0384; one name, two codes), and SC7006
`valid-use-server` (ADR 0385; upstream's placement checks and `clientWrappers`
are built, its acceptance of a re-export is not, and its wrapper list matches imports exactly). Decided and not yet shipped: `reactivity` as a group key over
SC1001, SC1002, SC1005 and SC1007 ([ADR 0386](../decisions.md#adr-0386)), each
member still configurable alone. Inline `eslint-disable` comments have no effect
until [ADR 0387](../decisions.md#adr-0387) ships.

Formatting, browser-policy, CSS-policy, compatibility-policy, and analyzer
bookkeeping rules were retired instead of being presented as certified runtime
defects (ADR 0110).
