# Rules

solid-checker ships one dialect-owned catalog: 34 identities for Solid 2.0,
with unprefixed rule names: 28 rule identities a project configures (SC2004 and
SC2005 share one name), two obligations
(SC9005 and SC9012, [ADR 0440](../decisions.md#adr-0440)) and four findings
decided before or beside the analysis (SC9013 to SC9016).
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

The `reactivity` key (upstream's name) sets `strict-read-untracked`,
`reactive-read-after-await`, `uncalled-accessor` and `reactive-handler-frozen`
together; a member the same file names keeps its own setting, whatever the key
order. In ESLint, `solid-checker/reactivity` is accepted and does nothing:
severity stays per member.

ESLint's generated `v1` and `v2` configs include every `prefer-*` rule.
Setting one to `off` opts out. The legacy `preferences-v1` and
`preferences-v2` configs remain available for compatibility and are now
redundant.

Renamed configuration keys transfer their setting through `RULE_ALIASES`, and
retired identities load as no-ops through `RETIRED_RULES`, both in
`rust/crates/solid-facts-backend/src/dialect.rs` ([ADR 0433](../decisions.md#adr-0433)).

## Solid 2.0 catalog — 28 rule identities and 4 detection findings

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
| SC9013 | [unsupported-solid-runtime](unsupported-solid-runtime.md) | error | on |
| SC9014 | [unaudited-solid-release](unaudited-solid-release.md) | warning | on |
| SC9015 | [compiler-refused-source](compiler-refused-source.md) | error (warning notice for an unproven refusal) | on |
| SC9016 | [unknown-suppression-rule](unknown-suppression-rule.md) | warning | on |

`http-response-after-flush`, `unsupported-solid-runtime`,
`unaudited-solid-release`, `compiler-refused-source` (for a refusal it does
not prove, ADR 0441) and `unknown-suppression-rule` can produce
uncertifiable results. Their pages name the missing evidence and remediation.

## Code no execution reaches

A violation at a site no execution reaches is uncertifiable, whatever the rule
([ADR 0458](../decisions.md#adr-0458)): the right of `false &&`, an
`if (false)` arm, code after a `return` or `throw` in the same block, or a
function nothing that runs calls, renders or names (also one an IIFE returns
into a discarded result, and a method of a class nothing names). A function at
module scope, and any value handed on, may run. Style rules (SC80xx) and
`flush-in-effect-callback` (SC2008) are unaffected.

## A binding named before its declarator

A violation at a site that names a `const` or `let` binding (a destructured
one included: `const [count, setCount] = createSignal(0)`) where the
declarator is not proven to have run is uncertifiable, whatever the rule
([ADR 0464](../decisions.md#adr-0464)): if the code runs first it throws a
`ReferenceError` instead. `[1].forEach(() => count())` written above the
declarator is one; the same callback below it is still reported. Style rules
(SC80xx) are unaffected.

## Obligations

An obligation is an internal uncertifiable result ([ADR 0328](../decisions.md#adr-0328)):
what the analysis could not establish, never a claim about the user's code. It
has no rule-options key, no ESLint rule and no switch, so nothing disables it
([ADR 0440](../decisions.md#adr-0440)). The human formats and ESLint do not
show it and `--certify` ignores it; `--format json` and `--internal` keep it,
with the `uncertifiable` status it sets.

| Code | Obligation | Raised when |
| --- | --- | --- |
| SC9005 | [package-behavior-unavailable](package-behavior-unavailable.md) | an imported package's required runtime behavior is unavailable from source |
| SC9012 | [reactive-dispatch-unresolved](reactive-dispatch-unresolved.md) | a type-correct call's runtime target, or a source handed to a package callee (context `package-callee`, formerly SC9011), cannot be established |

Every former name configures nothing and still loads, through `RETIRED_RULES` in
`rust/crates/solid-facts-backend/src/dialect.rs` and the matching no-op keys of
the ESLint plugin: `package-behavior-unavailable`, `package-contract-incomplete`
(SC9005's name before ADR 0312) and its three `package-contract-*` aliases,
`reactive-dispatch-unresolved`, and `reactive-source-uncaptured` (SC9011).
Package contracts and receipts are no longer consumed; see
[ADR 0312](../decisions.md#adr-0312).

`unsupported-solid-runtime`, `unaudited-solid-release`,
`compiler-refused-source` and `unknown-suppression-rule` are the four
identities here the rules engine never produces. Dialect detection decides the
first two, fact building the third, suppression comments the fourth.
The refusal replaces the analysis and appears **alone** when it appears at all
(ADR 0110). The notices accompany the analysis.

## Suppressing a finding

A comment removes the **violations at its site**. It never removes an
uncertifiable result (an obligation is not a defect the author can accept), a
notice, or a project-level finding.

```ts
// eslint-disable-next-line solid-checker/strict-read-untracked
console.log(doubled());
/* solid-checker-disable uncalled-accessor, SC1007 */
// ...
/* solid-checker-enable uncalled-accessor, SC1007 */
```

- Forms, with ESLint's grammar and meaning: `disable`, `enable`, `disable-line`
  and `disable-next-line`, in the `eslint-` and `solid-checker-` families
  (a `//` comment carries only the two line forms; names are separated by
  commas; ` -- reason` is ignored; a `-line` comment must stay on one line;
  `disable-next-line` counts from the comment's last line). No names means
  every checker rule.
- Names: `solid-checker/<rule>` in either family; `solid/<name>` for
  eslint-plugin-solid's pinned names (`solid/reactivity` is the group,
  `solid/no-destructure` SC1003); a `solid-checker-*` comment also takes the
  bare name and an SC code (`SC1001`). A rule's former name works, and
  `resolve-in-tracked-scope` and `until-in-tracked-scope` reach their own code
  only. `reactivity` reaches its four members (ADR 0428).
- The two families scan independently, as ESLint's comments and a comment it
  does not read do. A name from another plugin or a foreign prefix is not read.
- An obligation (SC9005, SC9012; [ADR 0440](../decisions.md#adr-0440)) is never
  suppressed. Its rule names, its codes and every retired name or code
  (`package-contract-*`, `reactive-source-uncaptured`, SC9011) are accepted
  without a notice and suppress nothing.
- A `solid-checker/*` name nothing resolves is reported as
  [unknown-suppression-rule](unknown-suppression-rule.md), and `--certify`
  fails on it.
- Lines are counted as ESLint counts them (`\n`, `\r\n`, `\r`, U+2028, U+2029),
  which can differ from a finding's reported line on a lone `\r`.
- Under ESLint the adapter forwards the rule keys it enables for the file and
  native skips `eslint-disable solid-checker/<key>` for those, because ESLint
  applies them to the key that reported. A member reported through
  `certification`, and `solid-checker/reactivity` (a no-op rule), are applied
  by native instead, and ESLint's own `reportUnusedDisableDirectives` then
  warns that the comment matched nothing it reported.

## Migrating from eslint-plugin-solid

A rule that maps onto an eslint-plugin-solid rule takes the upstream name
unless that name no longer describes what the checker proves, and its scope is
a superset of upstream's ([ADR 0381](../decisions.md#adr-0381)). Shipped renames (each old name is an alias, ADR 0433; the two names of a merged rule are limited to their own code, ADR 0426): SC2001
`no-write-in-pure-computation` (ADR 0424), SC2003
`no-store-mutation-outside-setter` (ADR 0425; upstream's array-method and
projection shapes are built, its `delete` is not: TS2790 on a required property), SC2004 and SC2005 together as
`no-await-primitive-in-tracked-scope` (ADR 0426; one name, two codes), and SC7006
`valid-use-server` (ADR 0427; upstream's placement checks and `clientWrappers`
are built, its acceptance of a re-export is not, and its wrapper list matches imports exactly). `reactivity` is a group key over
SC1001, SC1002, SC1005 and SC1007 ([ADR 0428](../decisions.md#adr-0428)), each
member still configurable alone. SC1003 stays outside it: upstream's `reactivity`
also reports destructured props, which the checker reports as
[no-destructure](no-destructure.md). Inline `eslint-disable` comments work
under both prefixes ([ADR 0429](../decisions.md#adr-0429); below).

Formatting, browser-policy, CSS-policy, compatibility-policy, and analyzer
bookkeeping rules were retired instead of being presented as certified runtime
defects (ADR 0110).
