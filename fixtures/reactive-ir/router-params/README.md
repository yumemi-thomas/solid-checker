# Route parameters are URL segments (ADR 0324)

`useParams()` of the audited `@solidjs/router@2.0.0-next.41` hands out URL path-segment strings, percent-decoded (or `undefined` for an absent optional one; `types.d.ts:41`), so a `<Show when={params.k}>` callback receives a string, settling the unmodeled use a callee makes of the prop it feeds (ADR 0320): the accessor and `keyed` forms have no `SC9012`; computed keys, `Object.prototype` names, aliases, wrapped/namespace `useParams`, reassigned bindings, `useSearchParams`, dynamic `keyed`, written/defined keys stay open.

`node_modules/@solidjs/router` is the published `@solidjs/router@2.0.0-next.41` byte for byte (MIT; `sha512-wCnhq37p4dpTYuy2Zz8nlf9Si/t4cMXUiMY0r7MxzHSH2c4jUiSrA8LkSdif6swwAIT/dOP7Pbv4hM1qY5ZOuA==`; digest `cec58599…cefcef`); other declarations are published rc.13. Siblings: `router-params-*` (most pin `next.26`).
