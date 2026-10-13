# A namespace call is a call of the declaration it resolves to

A call through `ns.describe` names the whole member expression, while the
helper's own symbol reference names only the `describe` property. That property
is accounted for as the call only when its exact semantic entity resolves to the
same declaration as the resolved call edge. The `as typeof ns.describe` wrapper
keeps the identity, and its type query is erased.

- `describe` is entered only by that call: its caller-supplied-member
  obligation clears in this closed application.
- `escapes` is read as a value (`export const handedOut = ns.escapes`): its
  obligation stays (`SC9012`).
- `shadow.tsx` has a local `ns` object with a `describe` method. The same
  spelling confers nothing, and it has no obligation of its own because its
  argument is visible.

The direct call does not clear a module that is also reachable as a
namespace object used other than by a static member (`escapes.ts`):

- `listed` stays (`SC9012`): `Object.values(listing)` enters it without
  naming it.
- `picked` stays: `picking["picked"]` records no reference to it.
- `loaded` stays: `import("./loaded")` hands out its namespace object.
- `viaBarrel` stays: `Object.values(reexported)` enumerates a barrel that
  re-exports it, so the barrel's namespace exposes it.
- `viaDefault` stays: `barrel3.ts` exports it as its default, and
  `Object.values(defaulted)` enumerates that barrel's namespace.
