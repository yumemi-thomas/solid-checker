# Open claims at call arguments, collapsed per package

Pins the collapse of open-claims `SC9005` raised at **call arguments**
(`unknown-contract-claims:callbacks`, a warning since ADR 0119): one finding per
`(package, open domains)` for the project (ADR 0224), not one per call or
per export.

The accepted contract leaves `callbacks` open for `runFirst` and `runSecond`
and closes it, empty, for `runClosed`. Every callable argument of an export
whose `callbacks` is open is an obligation, and its sentence names the package,
the export and the open domain and nothing about the call: the fix is closing
that domain in the contract, once. Measured on kobalte core
(`docs/package-contract-v2/phase22/2026-09-26-project-side-certification-on-kobalte-core.md`,
defect 7), reporting it per call turned one collapsed acceptance-gate finding
into 31 warnings when a partly closed contract was admitted.

| export | call sites | findings before | after |
| --- | --- | ---: | ---: |
| `runFirst`, `runSecond` | `App.ts` x3, `Other.ts` x3 | 6 | 1, anchored at `App.ts`'s first call |
| `runClosed` | `App.ts` x1 | 0 | 0, the control |

The surviving finding carries the other five sites in `relatedLocations`,
names both exports and says "(6 call sites)". It keeps `severity: warning`
and `kind: uncertifiable`, and names its subject as `subjectKind: "package"`,
which is what makes the ESLint adapter report it in `Other.ts` too. This
snapshot format records neither related locations nor messages, so what it
pins is the count and the anchor; the grouping, order and wording are pinned
by `projection::tests::open_claims_at_call_arguments_collapse_per_package_across_files`.

Two exports share one finding: closing `runFirst`'s domain does nothing for
`runSecond`, but both are fixed in the same place, the package's contract,
and the message names each of them.

`Other.ts` reaches the exports through `App.ts`'s re-export because an
authorized fixture's receipt binds exactly one importer
(`fixture_authorization.rs`); the calls there still resolve to the accepted
package exports, which is what makes the collapse cross-file.

## Why this fixture needs an accepted contract

Everything above is downstream of a contract the checker *accepted*, so the
fixture ships `.solid-checker/authorize-contract.json` rather than a catalog,
as `../package-member-callbacks-consumer` does. Un-authorized it certifies: the
package does not use Solid in its manifest, so no acceptance gate is raised, and
the authorization is what is under test.

## Stubs

`node_modules/reactive-package/package.json` is byte-identical to
`package-merged-props-consumer`'s, so the closure digest and package integrity
in the import block are the ones that fixture already pins. `index.d.ts` is
outside the closure; every call in the fixture is `tsc`-clean against it.
`solid-js.d.ts` is `package-member-callbacks-consumer`'s, and
`node_modules/solid-js` is required because an authorized fixture is analyzed
from a copy at a different depth.
