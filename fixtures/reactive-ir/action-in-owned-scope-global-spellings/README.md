# action-in-owned-scope-global-spellings

**Claim (ADR 0999).** A global a package writes is found whatever way its name, or the route to the global object, is spelled.

- **Project side.** An identifier may be written with Unicode escapes (`\u0062toa`, `\u{62}toa`), and the runtime reads the name, not the spelling. `fz-sbtoa`
  replaces `btoa`; a call of it spelled `\u0062toa("x")`, `\u{62}toa("x")`, `window.\u0062toa("x")` or `\u0077indow.btoa("x")` is the uncertifiable SC2002 row the plain
  call has (`package-may-call-action`). The name was compared as source text, so each was silent (Chrome rc.21 throws ACTION_CALLED_IN_OWNED_SCOPE).
- **Package side.** `x["defaultView"]` is `x.defaultView`: `fz-sview` reaches the global object through `document.documentElement.ownerDocument["defaultView"]` and
  replaces `atob` there. The scan read only the static member, so the write was not recorded and the project's `atob("x")` was silent.
- **Silent.** `unescape("x")` (no package writes it) and a local `const btoa`.

`node_modules/fz-*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of
`action-in-owned-scope-unread-packages`; `.solid-checker/runtime.json` closes the program boundary.

Chrome: `fixtures/runtime-oracle/package-global-escaped-name`, `package-global-defaultview`.
