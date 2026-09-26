# render-effect-apply-root

**Claim.** An owner-requiring operation in a `createRenderEffect` apply is
uncertifiable when the render effect is created inside a `createRoot` callback,
or inside any other owner-creating callback, exactly as when it is created in a
component body. The enclosing root answers the apply's first run only.

Measured on the published bytes, `@solidjs/signals@2.0.0-rc.3` and `2.0.0-rc.9`
(`solid-js` dev and prod client builds, the rc.3 install manifest-verified
against `benchmarks/package-contract-v2/phase0/rc3/`). Under `createRoot`, a
render effect whose compute reads a signal:

- first apply, during the call: `getOwner()` is the root; an `onCleanup` there
  runs when the root is disposed; no warning;
- every later apply, from the flush after the signal is written: `getOwner()`
  is `null`; an `onCleanup` there raises `NO_OWNER_CLEANUP` (dev) and never
  runs, not on the next apply and not on dispose; a render effect created
  there raises `NO_OWNER_EFFECT` (dev);
- with a constant compute no later apply happens, and the first run's cleanup
  runs on dispose.

So the operation is wrong only if a later apply happens, which needs a
compute source to change while the effect is alive. The checker proves no such
write, and server builds never re-run the apply at all, so the finding is
uncertifiable rather than a violation: the same decision, for the same reason,
as the component-body case in `render-effect-apply-timing`.

The owner passes used to answer every call lexically inside an owner-providing
region (a `createRoot` callback, a memo or effect compute, a `render` callback,
a row mapper, a compiler-owned region) as owned, without consulting the owner
graph. That lexical answer is now withheld where the containing function's
propagated context carries the later-run bit, which only a render-effect apply
edge introduces; the operation is then judged on the graph, where the bit makes
it uncertifiable. Both passes decide it through one function
(`owners::root_owned_at`).

| Case | Finding | Why |
| --- | --- | --- |
| cleanup in a render-effect apply, directly under `createRoot` | `SC4001` uncertifiable | first run owned by the root, later runs detached |
| effect created in that apply | `SC4001` uncertifiable | the same, for an effect |
| the namespace spelling | `SC4001` uncertifiable | resolves to the same primitives |
| cleanup in a render-effect apply under a memo compute under a root | `SC4001` uncertifiable | every owner-creating region is the same shortcut |
| cleanup in a helper declared in the root and called from the apply | `SC4001` uncertifiable | the helper runs in the apply's extent; the graph, not the helper's position, decides |
| cleanup in the root body, in a render-effect compute, in a helper only the root body calls | none | owned on every run |
| cleanup in a `createEffect` apply inside `createRoot` | none | **not this fixture's claim, and a known false negative**: that apply has no owner on any run (probed: `getOwner()` is `null`, `NO_OWNER_CLEANUP`, the cleanup never runs on dispose), but it carries no later-run bit, so the root still answers it lexically. Pinned so a change to it is deliberate |
| read in a `createRenderEffect` apply (named import, namespace, named function) | `SC1001` violation, message names `createRenderEffect` | the read context names the primitive whose apply it is |
| read in a `createEffect` apply | `SC1001` violation, message names `createEffect` | control |
| read in one function passed as the apply of both primitives | `SC1001` violation, message names neither | no single primitive to name |

This project keeps its messages in its snapshot (`KEEPS_WORDING` in
`scripts/coverage.mjs`): the read-context wording is part of the claim.

**Stub.** `solid-js.d.ts` is a byte copy of `render-effect-apply-timing`'s,
whose declarations are verbatim from the published rc.3 typings. `App.tsx`
type-checks cleanly against that stub and against the real
`solid-js`/`@solidjs/signals`/`@solidjs/web` rc.3 and rc.9 typings
(`tsc --noEmit`, TypeScript 5.9.3, `jsxImportSource: "@solidjs/web"`).
