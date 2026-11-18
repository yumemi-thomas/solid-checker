# handed-literal-prop-read

**Claim (ADR 1103).** A component prop read in an anonymous function literal handed whole to a project helper, inside a named local function the component body calls, is never silent. `discover_member` ended such a read as `placed-elsewhere`, trusting ADR 0936's fallback to the named function's call edges (`go()`), but the literal is entered by the helper's call and no summary carries a prop read of it to `go()`: no row stood anywhere (a live false clean, `mx-strict-read-untracked-ref-factory-body-memo-direct-helper-s-prop-value-cast`, exposed on a same-file component by ADR 1091). Such a read is now SC1001 uncertifiable (`read-unknown`, `handed-literal`).

| component | outcome |
|---|---|
| `Handed` | SC1001 uncertifiable at the read |
| `Direct` (the read in the named function itself) | SC1001 violation at `go()` |
| `BodyHanded` (the literal handed from the body) | SC1001 violation at the read |
| `HandedAccessor` (an accessor read in the handed literal) | SC1001 violation at `go()` |

Chrome: `fixtures/runtime-oracle/handed-literal-prop-read` (STRICT_READ_UNTRACKED at the read). `solid-js.d.ts` is `jsx-helper-child-component`'s.
