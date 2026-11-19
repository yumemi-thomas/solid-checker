# action-in-owned-scope-window-patterns

**Claim (ADR 1108 part 2).** A member the DOM types as this realm's window (`defaultView`, `view`, `source`, ADR 1108 part 1) read by an object pattern is the global object at every use of its binding: `({ view }) => { view.btoa = … }` writes `btoa`, and the binding handed on is an escape. The call of the written global is an uncertifiable site (`global-write:<name>:<root>`). Never a violation.

- **Uncertifiable**: `WrittenThroughViewPattern` (`btoa`, a click's `({ view })`), `WrittenThroughSourcePattern` (`atob` in a memo after the posted message, `({ data, source })`), `WrittenThroughDefaultViewPattern` (`unescape`, `const { defaultView } = el.ownerDocument`).
- **Uncertifiable, unchanged (part 1)**: `WrittenThroughViewMember` (`event.view.encodeURI = …`).
- **Silent**: `WrittenByClearPackage` (`fz-rclear` writes `decodeURI` through `({ view })` and names no Solid package), `Unwritten` (`encodeURIComponent`; `fz-rread` only reads `view.isSecureContext` off its pattern's binding, which writes nothing), `CalledInHandler`.

Chrome (rc.21, runtime oracle case `fixtures/runtime-oracle/package-window-patterns`): on the part 1 build 3 unproven-clean sites; here every site covered-uncertifiable.

`node_modules/fz-*` are **not** published bytes. The Solid stubs and the closed boundary are those of `action-in-owned-scope-prototype-writes`.
