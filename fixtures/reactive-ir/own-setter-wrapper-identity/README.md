# Own-setter write through a TypeScript-wrapped setter

Claim: a Type Facts entity demanded at a wrapped callee (`setStore!`,
`setStore as typeof setStore`) names the setter, because transparent
TypeScript wrappers preserve the value. The exact-identifier reference index
therefore peels them before testing identity, and a write through the store's
own proxy inside its own setter's draft callback stays silent. A write inside
another store's (wrapped) setter is still SC2003.

The project is an application (`"private": true`) on rc.13. `solid-js.d.ts`
adds the plain `createStore` form copied from published
`@solidjs/signals@2.0.0-rc.13`. The sources pass `tsc --noEmit` (strict,
skipLibCheck false) against installed rc.13; a browser/development runtime
probe of the asserted form commits the write (review artifact,
rust/target/research/context-member/REVIEW-1.md).
