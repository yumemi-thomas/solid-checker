# Route parameters are URL segments (ADR 0324)

`useParams()` of the audited `@solidjs/router@2.0.0-next.26` hands out URL path-segment strings (or `undefined` for an absent optional one; `types.d.ts:27`), so a `<Show when={params.k}>` callback receives a string, settling the unmodeled use a callee makes of the prop it feeds (ADR 0320): the accessor and `keyed` forms have no `SC9012`; computed keys, `Object.prototype` names, aliases, wrapped/namespace `useParams`, reassigned bindings, `useSearchParams`, dynamic `keyed`, written/defined keys stay open.

`node_modules/@solidjs/router` is the published `@solidjs/router@2.0.0-next.26` byte for byte (MIT; digest `e67af539…04d6`; `dist/index.js` sha256 `b657bc8d…baa35`, `routing.js` `639f16fd…153ea`, `utils.js` `562b280a…7e2a`); other declarations are published rc.13. Siblings: `router-params-*`.
