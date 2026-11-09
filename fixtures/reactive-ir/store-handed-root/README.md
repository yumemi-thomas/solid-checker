# store-handed-root

ADR 0919: a write through a container a caller handed in (a props member, a
destructured props slot, a `merge`/`omit` view, an own-props spread forward,
a context value, a hook's result or member, a tuple slot, an imported store's
alias) joins the container's value over every route. A `createStore` store on
every route, written outside every window that may be a setter's: violation
(`PropMember` … `Imported`, `Tuple`, `Shadowed`, whose local `useContext`
returns the store `other`). A plain value on every route (`PlainProp`,
`ContextPlain`), or a handed setter's draft (`Draft`): silent. Mixed routes
(`Mixed`), a replaced slot (`Replaced`), an exported component (`Exported`):
`unproven-container-root`. A window that may be the setter's (`InSetter`), a
link past the store (`Nested`) or a callback a project function hands on
(`HandedOn`), or a tag created inside a setter's callback (`InWindow`):
`store-write-unproven`. A namespace member (`Namespace`) stays
ADR 0860's SC9012. The cases type-check against the published rc.21 typings.
