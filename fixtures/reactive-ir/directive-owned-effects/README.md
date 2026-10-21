# directive-owned-effects

`missing-owner` (SC4001) for an effect created inside an intrinsic `ref` expression. The owner walk places each function literal the expression writes at its top level by where it runs (ADR 0345):

- the applied callback, written in value position, runs in the ref application with no owner: a violation (line 23, and the arrow element of the array on line 34; SC6001 reports the same creations);
- a function a call in the expression calls runs while the component renders, under the owner of the code around the expression. An IIFE, the body of a factory the expression calls (`tracked(count)`), a callback a dialect slot runs during its call (`untrack`, line 29), and an IIFE element of a ref array (line 33) are silent. The runtime agrees: `oracle-behaviour-primitive-in-directive`'s control is a factory call, and its effect is disposed with its element;
- at module scope the same IIFE has no owner, since nothing renders it: a violation (line 46).

Uncertifiable, never a violation: a generator in the ref (line 38; calling one runs none of its body, and the ref application never iterates it), and a callback handed to a project helper that no dialect slot describes (line 40; the helper may run it under an owner of its own). Both stay internal (ADR 0328).

`untrack`'s callback is silent for SC4001, but SC6001 still reports its creation as undecided (line 29). That is SC6001's own premise, not this fixture's claim.

Declaration stubs copy `@solidjs/signals@2.0.0-rc.13` `dist/types` for the signatures used, and the `Ref` alias of `@solidjs/web`'s `jsx.d.ts`.
