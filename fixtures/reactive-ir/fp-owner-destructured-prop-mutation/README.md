# fp-owner-destructured-prop-mutation

**Claim.** `no-direct-mutation` proves a dropped write only through the props
container. `const { program } = props` binds the *value* of one property, which
is whatever object the caller passed (here a plain mutable one), so
`program.blend = x` is not a write to the readonly props proxy. A whole-object
alias (`const alias = props`) is still the container. Since 2026-10-19 a
write through the container is uncertifiable: a 2.0 props object is the
compiler's plain object (a new key is written, a getter-backed key throws) or a
proxy that drops the write, and which one is not proven at the write.

| Case | Finding | Why |
| --- | --- | --- |
| `props.count = 2` | `no-direct-mutation` uncertifiable | write through the props object |
| `const alias = props; alias.count = 3` | `no-direct-mutation` uncertifiable | alias of the whole object |
| `const { program } = props; program.blend = ...` | none | the binding shape is a named property value |

Destructuring itself stays `no-destructure`'s business; this fixture only pins
the mutation claim. Stub as in `fp-owner-show-children-callback`; the file also
type-checks against the real rc.9 typings.
