# Composite reference withholding

Claim: proofs that withhold a finding keep reading a composite expression's
binding through the reference index. A Type Facts entity demanded at a
composite span names its leading token. That is not value identity, so
accessor provenance requires an exact identifier before it uses one
(`context-member-identity`). These withholding proofs still read it, and an
earlier attempt to drop such keys from the index globally turned each case
below into a false finding:

- `on-foo={absent ?? absent}` and its conditional form, with `absent = null`:
  no listener is installed, so there is no SC1007.
- `runWithOwner(owner ?? owner, …)` with the component's own `getOwner()`: the
  effect is owned, so there is no SC4001.

The project is an application (`"private": true`) on rc.13. `solid-js.d.ts`
extends the `context-member-identity` stub with an `on-*` attribute and
`getOwner`/`runWithOwner`, copied from published `@solidjs/signals@2.0.0-rc.13`
with an opaque `Owner`. The sources pass `tsc --noEmit` (strict, skipLibCheck
false) against installed rc.13.

Positive controls in the same project: a non-callable `on-foo={count}` is
SC1007, and `runWithOwner(null, …)` creating an effect is SC4001.
