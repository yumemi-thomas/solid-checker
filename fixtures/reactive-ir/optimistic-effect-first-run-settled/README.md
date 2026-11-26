# SC2009: an onSettled callback in a child component (ADR 1224)

The child's `onSettled` callback calls the prop in `render`'s tail flush (a tracked effect under its owner): a violation. The premises are `flush_mount_hold_free`. Chrome: `fixtures/runtime-oracle/optimistic-effect-first-run`.
