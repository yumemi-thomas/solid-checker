# `implementation-census-reads-fresh-target`

ADR 0153 item C (part 5): a `runtime-accessor-installation` hazard lifted per
export when the installation's target is an allocation its function makes, the
target leaves only as a return value, and the export can execute no operation
on it.

`implementation-census-reads`' `./owned` entrypoint is the case-wide half of
this: its proxy is allocated at module scope, so every export there keeps the
hazard. This fixture is the other half.

| entrypoint | export | expected |
| --- | --- | --- |
| `.` | `createView` | proposes `reads` bounded against both sites; the proxy it returns is operated on nowhere |
| `.` | `createBounds` | the same; its one nested-callback use of the target is the installation |
| `.` | `readWidth` | proposes the same bounds, and the census refuses the `defineProperty` bound: `"width" in measured` is an operation on that target inside it, and one that leaves no form of its own |
| `.` | `plainSum` | proposes and keeps the bounds; it touches neither target |
| `./escaping` | every export | proposes bounds, and the census refuses all: `createStore` also keeps its object in module state (`the target is assigned`) |

The generator cannot tell the halves apart -- it has no producer -- so it
proposes a bound for every accessor hazard of the closure on every export whose
inferred `reads` is closed, and `expected.json` pins that. The certifier's
census decides each bound against the producer's accessor-installation census
(`contract_certification.rs`,
`a_fresh_accessor_target_bounds_every_export_but_its_reader`).

`node_modules/solid-js` is the dialect-selection stub only; nothing here calls
a Solid primitive.
