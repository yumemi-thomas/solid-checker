# Route parameters are URL segments (ADR 0324)

Audited `useParams()` values are URL segment strings or `undefined` (`types.d.ts:41`), so a `<Show when={params.k}>` callback's string settles its callee's prop use (ADR 0320): no `SC9012` in accessor or `keyed` form. Open: computed or `Object.prototype` keys, aliases, wrapped/namespace `useParams`, reassignment, `useSearchParams`, dynamic `keyed`, written keys.

`node_modules/@solidjs/router` is published `@solidjs/router@2.0.0-next.41` bytes (MIT; `sha512-wCnhq37p4dpTYuy2Zz8nlf9Si/t4cMXUiMY0r7MxzHSH2c4jUiSrA8LkSdif6swwAIT/dOP7Pbv4hM1qY5ZOuA==`; digest `cec58599…cefcef`); other declarations: published rc.13. Siblings `router-params-*` pin `next.26`.
