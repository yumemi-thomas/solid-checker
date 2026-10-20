# render-effect-apply-root

**Claim.** An owner-requiring operation in a `createRenderEffect` apply inside `createRoot` or another owner-creating callback is `SC4001` **uncertifiable** (the root owns only the first apply; later applies have `getOwner() === null` and need a changing compute source); a `createEffect` apply is proven unowned so its cleanup is a violation; root-body and render-effect-compute cleanup is none. The `SC1001` read message names `createRenderEffect` or `createEffect` (ADR 0301). `solid-js.d.ts` is a byte copy of `render-effect-apply-timing`'s (published rc.3).
