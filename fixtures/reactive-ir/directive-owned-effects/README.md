# directive-owned-effects

`missing-owner` (SC4001) for an effect created inside an intrinsic `ref` expression. The owner walk places each function literal the expression writes at its top level by where it runs (ADR 0345):

- a value the ref application receives (the expression itself, an array element, a conditional branch or a logical operand) runs in the application with no owner: a violation (line 23, the arrow element of the array on line 34, and the logical operand on line 68; SC6001 reports the same creations);
- a function a call in the expression calls runs while the component renders, under the owner of the code around the expression. An IIFE, the body of a factory the expression calls (`tracked(count)`), a callback a dialect slot runs during its call (`untrack`, line 29), an IIFE element of a ref array (line 33), and a default parameter initializer, which runs in its function's call (line 60, rendered by `Host`), are silent. The runtime agrees: `oracle-behaviour-primitive-in-directive`'s control is a factory call, and its effect is disposed with its element;
- at module scope the same IIFE has no owner, since nothing renders it: a violation (line 46).

Uncertifiable, never a violation: a generator in the ref (line 38; calling one runs none of its body, and the ref application never iterates it), a callback handed to a project helper that no dialect slot describes (line 40; the helper may run it under an owner of its own), a getter's body that the expression reads (line 49), a tagged template's substitution (line 53), and an operand of `void` (line 57; never called). Both stay internal (ADR 0328).

`untrack`'s callback is silent for SC4001, but SC6001 still reports its creation as undecided (line 29). That is SC6001's own premise, not this fixture's claim. SC6001 uses the same value-position test for its apply phase, so the getter, template and `void` shapes are undecided there too.

Declaration stubs copy `@solidjs/signals@2.0.0-rc.13` `dist/types` for the signatures used, and the `Ref` alias of `@solidjs/web`'s `jsx.d.ts`.
