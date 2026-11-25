# SC2001: a render-effect write in a child component (ADR 1227)

The child's render effect runs its first apply at its call while the mount's creation pass renders it as a direct child of intrinsic elements (`eagerly_mounted`), whatever its props: the write throws REACTIVE_WRITE_IN_OWNED_SCOPE on rc.21, a violation. Chrome: `fixtures/runtime-oracle/render-apply-named-write`.
