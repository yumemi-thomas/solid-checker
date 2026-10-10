# Partial structural return consumer

ADR 0177 consumer controls. The authorized contract states two exports whose
returned container lists its members without closing the enumeration, the
shape certification publishes once it leaves an unprovable member `unknown`:

- `createTicker` returns `[accessor, unknown]`;
- `createPanel` returns `{ value: accessor, stop: unknown }`.

Each listed member is proven at its own position or key; an open enumeration
only says more members may exist. So:

| Case | Finding | Why |
| --- | --- | --- |
| `BadTupleRead`: `running()` read at the component's top level | `strict-read-untracked` violation | the listed accessor is proven |
| `GoodTupleRead`: the same read inside JSX | none | tracked |
| `UnknownTupleMember`: calling the `unknown` member | `reactive-dispatch-unresolved` uncertifiable (ADR 0234) | its behavior is not described |
| `UnlistedPosition`: an index past the listed members | no violation | names no leaf |
| `BadObjectRead` / `GoodObjectRead` | violation / none | as for the tuple |
| `UnknownObjectMember` | `reactive-dispatch-unresolved` uncertifiable at `panel.stop` | as for the tuple |
| `UnknownMemberAsHandler`: the member as an `onClick` value | none | it runs later, outside the render |
| `EscapedTicker`: the whole return passed to a local function | `reactive-dispatch-unresolved` uncertifiable at the call | the member may be invoked wherever the value goes |

The fixture issuer supplies the contract; this fixture does not claim runtime
census coverage of any published package. The manifest bytes are
`fixed-structural-return-consumer`'s, so the closure digest is unchanged. The
declarations type the members as plain functions, which is what a published
package's typings would state for an accessor and a stopper.
