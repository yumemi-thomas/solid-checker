# owner-namespace-named-reads

**Claim (ADR 0347 amendment).** In a closed program, a dynamic import's result hands out only the exports the program reads by name:
- `lazy(() => import(x), options)` reads the one export its options name, `default` when they name none. Solid's rc.13 `lazy` reads `mod[options.export]`, or `mod.default` (`solid-js` `dist/solid.dev.js`, `lazy`). The callee must be `lazy` imported from `solid-js`.
- `const { a } = await import(x)` and `(await import(x)).a` read `a`.

A named export is handed out by name, so a detached callback it passes to `runWithOwner(null, …)` runs with no owner: a violation. Any other export of the module stays dormant. A result whose read is not named (here `Object.keys`) hands out every export with no name, so an export nothing names is uncertain, never a violation.

Reported:
- `Named` (`page.tsx`), read by `lazy(() => import("./page"), { export: "Named" })`.
- `reached` (`other.ts`), read by `const { reached } = await import("./other")`.
- `handedOut` (`member.ts`), read by `(await import("./member")).handedOut`.

Silent, dormant: `helper` (`page.tsx`), because `lazy` reads only `default` and `Named`; `neverCalled` (`other.ts`) and `unread` (`member.ts`), which nothing reads.

Uncertifiable, not a violation: `escaped` (`escapes.ts`), whose namespace escapes through `Object.keys(await import("./escapes"))`.

The stub is `component-owner-binding`'s declarations plus the published rc.13 `Component` type and `lazy` overloads (`types/client/component.d.ts`).
