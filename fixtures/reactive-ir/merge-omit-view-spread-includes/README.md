# merge-omit-view-spread-includes

**Claim (ADR 1129, review F2).** rc.21's `omit` hides its keys with `hidden.includes(key)` (`@solidjs/signals@2.0.0-rc.21` `dist/dev.js` `isHidden`, 8766), over the rest array `omit(props, ...keys)` makes, so an `omit` view proof rests on `Array.prototype.includes`. A program that may replace it gets no `omit` view proved: a write to a member named `includes` on any receiver, a write through an unproven or non-DOM host prototype (ADR 0314), or a reflective write through a `prototype` (ADR 0292), the question ADR 0338 asks for `Function.prototype.bind`.

- `App.tsx`: `Array.prototype.includes = …` never hides `children`; `Leaky`'s `omit(props, "children")` spread stays SC8003 uncertifiable (`spread-content-unproven`): its caller's children reach the div beside `innerHTML`. `MergedOnly` (a `merge` view, which asks no `includes`) is proved.
- `variants/define`: `Object.defineProperty(Array.prototype, "includes", …)`: uncertifiable.
- `variants/alias`: `const proto = Array.prototype; proto.includes = …`: uncertifiable.
- `variants/read-only` (negative): `includes` is called, never written: `Leaky` is proved, clean.

On 379f2f79e (ADR 1129 round 0) the three replaced forms were clean. Stubs and pinned sources as in `merge-omit-view-spread`; Chrome runs every variant (`scripts/runtime-oracle.mjs --cases fixtures/reactive-ir --only merge-omit-view-spread-includes`).
