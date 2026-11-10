# stored-throw-wrapper-boundary

**Claim (ADR 0937).** A provider-less `useContext` read mounted as a user component's child or prop throws where the component
renders that prop. U (`computation-error-unobserved`): `Real`, `Call`, `Alias`, `Nested`, `RenderProp`, `ContentProp`,
`RowUnderBoundary`. V: `Plain`, `Outside`, `Foreign`, `Destructured`, `ReadBefore`, `Shadowed`, `RowUnhandled`. Chrome (rc.21)
reports CONTEXT_NOT_FOUND at each V only.
