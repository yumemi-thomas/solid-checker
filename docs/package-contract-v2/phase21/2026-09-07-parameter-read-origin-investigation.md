# Parameter-read origin prerequisite

The Floating UI investigation exposed an accepted positive read claim whose
input is not the caller's value. This is a prerequisite to extending coverage,
not permission to make the current value-path gate more permissive.

The [retained counterexample](2026-09-07-parameter-read-origin-counterexample.json)
contains the exact temporary package, archive digest, published catalog entry,
accepted document/receipt digests, and certification audit. Its complete body is:

```js
export function read(input) {
  input = { foo() { return 1; } };
  return input.foo();
}
```

Native certification and ordinary trusted catalog discovery succeed. The
accepted main claims a possible `read-0` whose input is parameter 0, path
`["foo"]`. A valid typed proxy argument records zero member accesses during
the export, which returns 1. The same proxy records one member access when
the control calls its own `foo`, returning 2. TypeScript 5.9.3 reports zero
diagnostics against the package's declaration. The body unconditionally
replaces the binding before its only member read, so the issue is not merely
an unsampled branch or an imprecise runtime observation.

`parameterCensusRootsLocked` records parameter bindings, and
`parameterValueSourceLocked` follows their syntactic member chains even after
assignment. Those records establish a binding and path, not unchanged value
identity. Existing read-family evidence can consume them while recursive value
shape is satisfied by the caller's declared member type. Neither proves that
the read still observes that caller's value.

The immediate correction must require affirmative input-origin evidence. The
existing `unwrittenParameters` premise is sufficient for the unchanged-binding
subset; an absent row is not permission. The later Floating UI extension needs
a separate positive proof for the caller/fresh-default join, including exact
symbols, each possible write, the actual read path, execution reachability and
the operation's zero lower bound. An unconditional local replacement must
remain excluded. Stronger cardinalities, aliases, captured writes, defaults,
rest/destructuring and unsupported control flow require their own premises.

The other semantic task was rechecked and remains archived; a coordination
message was rejected as archived. No new protocol or receipt interface has
been changed during this investigation. These diagnostic artifacts do not add
an ecosystem entrypoint or change its denominator.
