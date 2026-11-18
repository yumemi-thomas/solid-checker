# A created anchor's click and remove reach no JSX handler

ADR 1099. The anchor is created, appended to `document.body`, clicked and removed (ADR 0640's receiver). Its path is the anchor,
`body` and `html`: Solid's delegated handler walks that path and finds no JSX element, so neither call delivers to the button's
handler. The handler runs only from a later click, with no owner: `useContext` throws `NoOwnerError` (SC4001 violation) before any
`ContextNotFoundError`, so SC4002 is silent. Chrome: `runtime-oracle/created-anchor-unowned-handler`.
