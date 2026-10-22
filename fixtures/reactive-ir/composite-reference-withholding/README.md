# Composite reference withholding

Proofs that withhold a finding keep reading a composite expression's binding through the reference index (a Type Facts entity at a composite span names its leading token): `onFoo-bar={absent ?? absent}` with `absent = null` installs no listener (no SC1007); `runWithOwner(owner ?? owner, ...)` with the component's own `getOwner()` is owned (no SC4001). Controls: a non-callable `onFoo-bar={count}` is SC1007; `runWithOwner(null, ...)` creating an effect is SC4001. `getOwner`/`runWithOwner` are copied from published `@solidjs/signals@2.0.0-rc.13`.
