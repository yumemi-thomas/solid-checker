# SC2009: a named callback prop in a child effect apply (ADR 1224)

The parent hands a function it declares by name; the child's `createEffect` apply calls it in `render`'s tail flush: a violation. Chrome: `fixtures/runtime-oracle/optimistic-effect-first-run`.
