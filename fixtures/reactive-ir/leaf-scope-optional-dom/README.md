# leaf-scope-optional-dom

**Claim (ADR 0397).** An optional call (`el?.m()`, also on a `let` typed `T | undefined`) is read like its non-optional form: any exact `lib.dom` member is none except a synchronous dispatcher (`focus`, `click`, `dispatchEvent`, `showModal`; ADR 0329), the optional `getContext` of `HTMLCanvasElement` and `OffscreenCanvas` (WebGL's creation event; the plain call stays trusted) and `removeEventListener` (no audited timing), which are `SC9012`.
