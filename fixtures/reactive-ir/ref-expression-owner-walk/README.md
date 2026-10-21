# ref-expression-owner-walk

`missing-owner` (SC4001) and `primitive-in-directive-application` (SC6001) for an effect created in a function an intrinsic `ref` expression runs. The owner walk places each function the expression runs (ADR 0345):

- the right operand of `&&` is a value the ref application receives: a violation (line 10, the control for the next case);
- the left operand of `&&` is tested and never applied, since a function is always truthy (ADR 0345 review A). The value received is `noop`, so the arrow's owner is unproven: uncertifiable, for SC4001 and SC6001 (line 15);
- a component's children run inside the component's own memo. The arrow element there is applied with no owner: a violation (line 24). The IIFE a ref in the children calls is unproven, not a violation, because its owner is Show's memo (line 25, SC4001);
- a `runWithOwner(null, fn)` callback runs with no owner, so its own effect does (line 32). A ref IIFE inside such a callback runs with none too (line 42). This fixture has no application manifest, so an exported function is open-world and the callback is proven unowned. In a closed program (an application manifest) an exported function nothing in the program calls is context-free, and the same IIFE is uncertifiable instead (see docs/open-work.md);
- under `createRoot`'s owner the IIFE's effect is owned by the root: silent (line 37, the negative control);
- at module initialization no owner is current, so the IIFE a module-scope wrapper calls, and the ref IIFE it contains, run with none: a violation (line 47). This is a call of a function literal at module scope (ADR 0345, `runs_at_module_init`);
- the ref IIFE in a method whose call the graph does not resolve (`new Builder().build()`) has no owner edge: its owner is unknown, so it is uncertifiable (line 54).

Stub: `solid-js.d.ts` copies the `solid-js` declarations of `fp-owner-show-children-callback` (published 2.0.0-rc.9 typings: `Show`'s non-keyed overloads, the signal, effect and element types), with a `ref` on `div` and the `Ref` alias of `@solidjs/web`'s `jsx.d.ts`. It adds `createRoot`, `runWithOwner` and `untrack` from `@solidjs/signals` 2.0.0-rc.13 (`core/owner.d.ts`, `core/core.d.ts`). `Owner` is declared as `object`: every call here passes `null`, which the published `Owner | null` accepts.
