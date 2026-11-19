# merge-omit-view-spread-includes

**Claim (ADR 1129, review F2).** rc.21's `omit` hides its keys with `hidden.includes(key)` (`@solidjs/signals@2.0.0-rc.21` `dist/dev.js` `isHidden`, 8766), over the rest array `omit(props, ...keys)` makes, so an `omit` view proof rests on `Array.prototype.includes`. The proof asks the shared standard-member question for `Array.includes` (`StandardMemberReplaced`): a file of the program that may replace it (ADRs 0781, 0876, 0939: a write of a member of that name on any receiver, by assignment, alias, definer, `Reflect.set` or `Object.assign`, a write a prototype chain reaches, a root handed to a writer), or a loaded package whose closure writes a built-in prototype's `includes` or any key (ADR 1108's index, clear packages included), withholds it.

The props type declares `textContent`, not `innerHTML`: the declared keys of `rest` (ADR 0436) then hold a content key other than the element's own `innerHTML`, so without the view the spread stays open (`spread-content-unproven`), and only the view proof can close it. With `innerHTML` declared, the declared key and the attribute are one slot and the element is clean with or without the proof, so the fixture would show nothing.

- `App.tsx`: `Array.prototype.includes = …` never hides `children`; `Leaky`'s `omit(props, "children")` spread stays SC8003 uncertifiable (`spread-content-unproven`): its caller's children reach the div beside `innerHTML`. `MergedOnly` (a `merge` view, which asks no `includes`) is proved.
- `variants/define`: `Object.defineProperty(Array.prototype, "includes", …)`: uncertifiable.
- `variants/alias`: `const proto = Array.prototype; proto.includes = …`: uncertifiable.
- `variants/define-alias`: `Object.defineProperty(proto, "includes", …)` through that alias, no `prototype` spelled at the call: uncertifiable (the round-1 census, ADR 0314 and ADR 0292's text test, certified it).
- `variants/package-writer`: `node_modules/fz-includes` (written for this fixture, not published bytes) replaces `Array.prototype.includes` where it loads and names no Solid package: uncertifiable.
- `variants/read-only` (negative): `includes` is called, never written: `Leaky` is proved, clean.

On 379f2f79e (ADR 1129 round 0) every replaced form was clean. Stubs and pinned sources as in `merge-omit-view-spread`; Chrome runs every variant (`scripts/runtime-oracle.mjs --cases fixtures/reactive-ir --only merge-omit-view-spread-includes`).
