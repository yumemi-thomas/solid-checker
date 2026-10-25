# unknown-suppression-rule

`SC9016` · **warning** · uncertifiable

A suppression comment names a `solid-checker/*` rule this checker does not
have, so it suppresses nothing for that name
([ADR 0387](../decisions.md#adr-0387)).

## What it does

The checker reads `eslint-disable`-family comments naming `solid-checker/<name>`
and its own `solid-checker-disable`-family comments (see
[Suppressing a finding](README.md#suppressing-a-finding)). A name that no rule,
former name, group (`reactivity`) or, in a `solid-checker-*` comment, SC code
resolves to is almost always a typo, and a typo is not silently ignored: the
finding it was meant to suppress is still reported, and this notice names the
unknown name.

- One notice per distinct name, at its first use; the other uses are related
  locations.
- It is project-level, like SC9014 and SC9015, so a run that would otherwise
  look clean shows it and `--certify` fails.
- Not an error for a name that is simply not ours: another plugin's
  (`@typescript-eslint/no-unused-vars`), eslint-plugin-solid's `solid/*` names
  (pinned to v0.18.1, `UPSTREAM_RULE_NAMES`), the adapter's `certification`,
  `package-note` and `contract-note`, and every retired or former name (the
  obligations' included: `package-behavior-unavailable`,
  `reactive-dispatch-unresolved`, SC9005, SC9011, SC9012; they suppress
  nothing, [ADR 0398](../decisions.md#adr-0398)) are accepted silently. `solid/*` names outside that list are not read at all.
- `eslint-disable` separates names with commas: `a b` is one name, `a b`, and
  is reported.

## Example

```ts
// eslint-disable-next-line solid-checker/strict-read-untrackd
console.log(doubled());
```

## How to fix

Spell the name as it appears in [the rule table](README.md), or use a former
name, `reactivity`, or an SC code (`solid-checker-disable-next-line SC1001`).

## Suppression

There is no site to suppress, and disabling the rule in the options file has
no effect: the notice is added after rule enablement, like SC9014.
