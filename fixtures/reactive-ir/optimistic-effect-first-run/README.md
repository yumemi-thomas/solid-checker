# SC2009: an effect apply in a child component (ADR 1224)

The child's `createEffect` apply calls the prop in `render`'s tail flush: a violation. The premises are `flush_mount_hold_free`: a module-scope `render` first, static trees, bodies that run nothing else, the child rendered once as a direct child of intrinsic elements with function-literal props. Chrome: `fixtures/runtime-oracle/optimistic-effect-first-run`.
