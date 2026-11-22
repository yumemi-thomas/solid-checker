# iife-prop-read

**Claim (ADR 1184).** A component prop read in an anonymous literal called where it is written (an IIFE, arrow or function
expression), inside a named local function the render calls, is never silent. `discover_member` ended such a read as
`placed-elsewhere`, trusting ADR 0936's fallback to the named function's call edges. No summary carries a prop read of the IIFE to
those calls, so no row stood anywhere. That was a certified false clean: census `mx-strict-read-untracked-ref-factory-body-signal-direct-iife-s-prop-value-cast`,
the `RefFactory` shape here. Such a read is now SC1001 uncertifiable, the same as ADR 1103's handed literal.

| component | outcome |
|---|---|
| `Arrow`, `Expression` | SC1001 violation at `go()` (ADR 1187; ADR 1184: uncertifiable at the read; main before it: clean) |
| `RefFactory` | SC1001 uncertifiable at the read, as main leaves the same read written without the IIFE (the `ref={dir()}` call is not placed) |
| `Direct` (the read in the named function) | SC1001 violation at `go()` |
| `BodyIife` (an IIFE in the body) | SC1001 violation at the read |
| `AccessorIife` (an accessor read in the IIFE) | SC1001 violation at `go()` |
| `Untracked`, `PlainObject` | clean |

Chrome: `fixtures/runtime-oracle/iife-prop-read`. STRICT_READ_UNTRACKED is raised at each read; the three IIFE reads are
covered-uncertifiable, the three controls covered-violation, and the clean controls are quiet.

**ADR 1187.** The helper pass (ADR 0933) charges a prop read in an IIFE written directly in a named local helper's body to the helper's
calls, as it does one written in the body: `Arrow` and `Expression` are violations at `go()`.
