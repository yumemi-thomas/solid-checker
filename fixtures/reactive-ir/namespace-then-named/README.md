# namespace-then-named

**Claim (ADR 1071).** `import("./barrel").then((m) => m.createPlain())` reads one name of the module it settles with (facts schema 97: a
`.then` callback whose one parameter is only the object of static members is a `Members` consumer). `createPlain` is a re-export of
`holder.ts`'s function, followed through the barrel (`exported_function`), so the load hands out `createPlain` only. Nothing else names
`createLoader`, so `useIsMounted`'s callers are all in view (none) and its `onSettled` needs no owner proof: no row. Before ADR 1071 the load
handed out the whole barrel: SC4001 `owner-callers-unenumerated` at `holder.ts:6`.

Twin: `namespace-then-escape` (the callback hands `m` on: the row stays).
