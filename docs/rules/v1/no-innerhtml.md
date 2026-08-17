# v1/no-innerhtml

`SC8008` · **error** · violation

## Scope: the React prop is TypeScript's on an intrinsic element

Narrowed on 2026-08-17 under the absolute rule in
[AGENTS.md](../../../AGENTS.md): never report what TypeScript already reports.

`dangerouslySetInnerHTML` is not in `JSX.IntrinsicElements`, so on a DOM element
it is already a type error against the real `solid-js@1.9.14` typings:

```
TS2322: Property 'dangerouslySetInnerHTML' does not exist on type 'HTMLAttributes<HTMLDivElement>'.
```

That is word for word this arm's own claim — *the prop is not supported*. The
duplication was confirmed by `scripts/parity-tsc-ownership.mjs`, which matched the
finding's span against the diagnostic's.

The arm survives on a **component**, whose props are whatever it declares: there
the prop is a permitted key, TypeScript is silent, and the claim that Solid's
renderer has no special case for the React name — so it arrives as an inert
attribute — is the only one available. The `innerHTML` rewrite fix rides that
half.

**The `innerHTML` arm is untouched.** `innerHTML` is a *declared* Solid prop, so
every claim about it — the injection surface, the conflict with JSX children, the
not-actually-markup advice — is independent on every element.

Validates `innerHTML` and React-style dangerous HTML properties from Oxc JSX
facts.

## Options

Configured in the project's `.solid-checker/rule-options.json` (see
[the rules index](../README.md#rule-options)):

```json
{
  "schemaVersion": 1,
  "rules": {
    "v1/no-innerhtml": { "allowStatic": true }
  }
}
```

- `allowStatic` (default `true`) — accept a value proven to be a static HTML
  string, whether written as a literal or proven through its TypeScript
  string-literal type. With `false`, every `innerHTML` value is reported as an
  injection surface.
