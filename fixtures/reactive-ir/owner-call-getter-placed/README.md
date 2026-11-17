# owner-call-getter-placed

**Claim (ADR 1031).** A component element's children, or a prop's value, are a getter that runs where the component reads the prop (`owner-call-jsx-getter`). Where
the tag resolves to one project component that takes a plain `props` identifier, every reference to it is a non-computed member read (or the argument of an
`omit` whose result is only spread on intrinsic elements), and every read of that prop is directly in the component's body, in the JSX of intrinsic elements or
of in-place renderers (`Show`, a context provider) written there, or in the function of a `children()` helper, the getter runs at those reads: each of the
site's code groups (SC3001, SC6002) is answered as for code written at each read, and ends only where every read that may run ends it.

| component | outcome | why |
|---|---|---|
| `InJsx` (`<p>{props.children}</p>`), `InBody` (read in the body), `ViaHelper` (`children(() => props.children)`), `SlotInJsx` (a prop's value), `ThroughShow` (`<Show>{props.children}</Show>`), `Spreads` (`omit(props, "class")` spread on a `<div>`) | the `<Inner />` they are handed is silent (was SC3001 + SC6002 `getter-runs-where-read`) | the reads run under the component's render owner, where the owner graph and the role engine answer as for any code written there |
| `OnApply` (an effect's apply reads it: the counterexample), `InLeaf` (a tracked effect's callback), `InHandler` (a handler) | the same two rows | a read in a nested function runs where that function runs: Chrome rc.21 throws PRIMITIVE_IN_EFFECT_CALLBACK and PRIMITIVE_IN_FORBIDDEN_SCOPE at the first two |
| `Escapes` (`props` handed to a function), `Destructured` (a pattern parameter), `SpreadEscapes` (`omit`'s view handed on too) | the same two rows | the read set is not known |
| `Ignores` (never reads them), `SpreadOmits` (`omit(props, "children")`, read nowhere else) | the same two rows | the getter never runs: no placement for code that never runs (ADR 1021's ruling) |

Chrome: `fixtures/runtime-oracle/owner-call-getter-placed`, 0 typing errors; nothing at the placed getters, the two throws covered by uncertifiable rows.
Stubs: `owner-call-jsx-getter`'s, with `children` and `ChildrenReturn` added from `solid-js@2.0.0-rc.21` `types/client/core.d.ts:128-130, 149`, and `omit` from
`@solidjs/signals@2.0.0-rc.21` `dist/types/store/utils.d.ts:260-261`.
