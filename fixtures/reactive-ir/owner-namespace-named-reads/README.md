# owner-namespace-named-reads

**Claim (ADR 0347).** In a closed program, a dynamic import's result hands out only the exports the program reads by name:
- `lazy(() => import(x), options)` reads the export its options name, `default` when none (rc.13 `lazy` reads `mod[options.export]` or `mod.default`; `solid-js` `dist/solid.dev.js`); the callee must be `lazy` imported from `solid-js`.
- `const { a } = await import(x)` and `(await import(x)).a` read `a`.

A named export's detached callback under `runWithOwner(null, …)` runs with no owner: a violation. Other exports stay dormant; an unnamed read (`Object.keys`) hands out every export, so an export nothing names is uncertain.

Reported: `Named` (`page.tsx`, via `lazy(…, { export: "Named" })`), `reached` (`other.ts`), `handedOut` (`member.ts`).

Silent: `helper` (`page.tsx`; `lazy` reads only `default` and `Named`), `neverCalled` (`other.ts`), `unread` (`member.ts`).

Uncertain: `escaped` (`escapes.ts`), whose namespace escapes through `Object.keys`.

Stubs: `component-owner-binding`'s declarations plus rc.13 `Component` and `lazy` overloads (`types/client/component.d.ts`).
