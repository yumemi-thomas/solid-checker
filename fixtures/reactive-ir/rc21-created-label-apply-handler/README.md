# A helper only an effect apply calls dispatches with no owner

ADR 1099. `pick` is a `const` arrow inside the component whose every reference is a call written directly in a `createEffect`
apply, so its label click runs on the apply's stack. rc.21 runs an apply with no `runWithOwner`, and no flush here drains under an
owner, so the button's handler runs with no owner on every delivery: SC4001 violation, SC4002 silent. Chrome:
`runtime-oracle/created-label-apply-handler`.
