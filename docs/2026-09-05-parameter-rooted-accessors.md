# Parameter-rooted accessors in the `creates` census — 2026-09-05

ADR 0034 was written before implementation. It extends the census's
`parameter-rooted` principle — the caller's object is the caller's code — from
the calls ADR 0008 already dispositioned to the read accessors those calls sit
beside, and adds a reviewed this-protocol table so
`Object.prototype.toString.call(value)` is decided by what that member reaches
rather than by the by-reference owner rule. Nothing here creates an ordinary
package contract, weakens a veto, or admits a host global.

## What the census now decides

A `get-accessor` or `property-access-unknown-accessor` form is dispositioned
`parameter-rooted-accessor` when the producer states the frame parameter its
receiver chain roots at. The producer states that index only when every premise
holds at once:

- the form sits on a property or element access in read position — not an
  assignment target, not the operand of `delete`;
- the receiver chain, after identity-preserving unwrapping, consists only of
  property, element and optional-chain reads down to a plain identifier;
- that identifier is a parameter of the implementation with no initializer and
  no rest token, and its symbol is assigned nowhere in the file;
- the declaration mentions neither `arguments` nor `eval`.

A `.call`/`.apply` whose resolved callee is a default-library
`Function.prototype` member additionally states its receiver's declaration and
the parameter its `this` argument roots at. The verifier consults
`CENSUS_THIS_PROTOCOL_MEMBERS` before the by-reference owner refusal; the table
holds one row, `Object.prototype.toString` (qualified `Object.toString`), whose
only reach into the argument is `Get(this, @@toStringTag)`, and it admits only a
rooted `this`. Everything else keeps its refusal by name: setters, writes,
`delete`, iteration, coercion, `instanceof`, a nested callable's own parameter,
a binding alias, a written parameter, a module-level receiver, `.call` on a
non-library receiver, and `.call` on a library member outside the table.

The producer states facts and decides nothing; the handshake protocol moves from
17 to 18 so a verifier expecting the fact refuses a producer that cannot state
it, instead of reading an absent field as "not rooted".

## Generator alignment

The proposal walk used to decline a member callee on a parameter as
`parameter-rooted` because the census would have refused the accessor form it
planned. That decline is now withheld for exactly the shape the census admits: a
direct, un-aliased, uninitialized identifier parameter of the *outermost*
function containing the call. Alias, nested-parameter and computed shapes keep
declining, pinned by `creates-decline-records`'s `parameterAliasRooted` and by
`implementation-census-creates`'s `nestedCallableParameterRead`.

Across the corpus this removed 29 `parameter-rooted` declines and the 15
`refusing-callee-fixpoint` declines that depended on them, across fifteen
fixtures. The only other movement in any `expected.json` or
`expected-proposal.json` is the added `creates` closure label on those exports;
`expected-refusals.json` sidecars whose last decline left were removed. Every
change was reviewed in the non-updating run before `--update`, and the
non-updating corpus is stable afterwards (94 fixtures, 43 declined closure
proposals).

## Fixtures

`implementation-census-creates` is the positive and negative pin in one place:

| export | outcome | what it pins |
| --- | --- | --- |
| `memberParameterRooted` | certifies | the direct parameter read |
| `toStringTagViaCall` | certifies | the this-protocol table row |
| `writtenBeforeRead`, `writtenAfterRead` | refuses | any write to the parameter, in either order |
| `moduleReceiverRead` | refuses | a module-level receiver is not the caller's object |
| `nestedCallableParameterRead` | refuses | a nested callable's own parameter |
| `setterOnParameter` | refuses | write position |
| `callNonLibraryReceiver` | refuses | `.call` on a non-library receiver |
| `callLibraryOutsideTable` | refuses | `Array.prototype.slice.call`, a member the table does not hold |

The two certifying exports run through checked recipes
(`probe-recipes/member-parameter-rooted.mjs`, `probe-recipes/to-string-tag.mjs`)
under the ordinary probe gate, so the disposition is exercised end to end, not
only in the verifier's unit tests. `creates-decline-records` keeps its
`parameterRooted` export as the control and no longer records a decline for it.

## Measurement

Run against the rebuilt checker with the protocol-18 producer, reusing the
authenticated planning of the earlier import-free diagnostics
(`/private/tmp/parameter-rooted-accessor-measurement/results.json`, SHA-256
`d625d54f15618aed4b73ebd300cdf872f1d1f8402a579c7f41e0ff7326589277`):

| case | profile | result |
| --- | --- | --- |
| `@kobalte/utils@0.9.2` `isString` | `node-strip-import-free-esm-v1` | certified, 1 accepted closure, receipt v6 |
| `@kobalte/utils@0.9.2` `isPointInPolygon` | `node-strip-import-free-esm-v1` | certified, 1 accepted closure, receipt v6 |
| `@kobalte/utils@2.0.0-alpha.0` `isPointInPolygon` | `node-strip-import-free-esm-v1` | certified, 1 accepted closure, receipt v6 |
| `@kobalte/utils@0.9.2` `scrollIntoView` | `node-strip-relative-ts-graph-esm-v1` | refused: `property-access-unknown-accessor (ElementAccessExpression)` at `scroll-into-view.ts:1768..1779` |
| `@kobalte/utils@0.9.2` `scrollIntoViewport` | `node-strip-relative-ts-graph-esm-v1` | refused, same form |

Three of the three targeted native refusals complete a controlled profile with
no contradiction. The two `scrollIntoView` controls refuse exactly where the ADR
predicted: `relativeOffset` is reassigned in its file, so the producer roots
nothing on it and the census keeps refusing the indexed read. The
flow-sensitive extension that would admit a parameter written only after its
last read is named in the ADR and not taken.

Standing: 26 of the original 40 TypeScript candidates now complete a controlled
profile. Fourteen refusals remain — the `scrollIntoView` pair on the written
binding and twelve host-realm refusals this ADR does not touch.

## Ordinary three-row baseline

The checked recipe corpus was rerun against the rebuilt checker
(`/private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/1ea56a4b-1438-48b4-8413-f4a008271f5f/scratchpad/three-row-adr0034/after.json`,
SHA-256 `c38f7531682af84f2037061d3fb2c1f4765d2aea57258ec2a105114b0b9ae34a`).
Every row keeps its class, reason and withheld-closure count from the ADR 0033
baseline, as expected: the alpha row's existing receipt identity is untouched,
the 0.9.2 row still stops at its mandatory probe gate, and the i18n row's
refusal is a `SpreadAssignment` form, which is not a read accessor and is not
admitted here.

| row | outcome | withheld closures | `exportsProven` |
| --- | --- | ---: | ---: |
| `@kobalte/utils@0.9.2\|solid1\|only` | mandatory probe gate `a9c9b71f…` did not complete | 0 | 0 |
| `@kobalte/utils@2.0.0-alpha.0\|solid2\|only` | certified; existing JavaScript closures | 11 | 0 |
| `@solid-primitives/i18n@2.2.1\|solid1\|only` | independent accessor-census refusal (`SpreadAssignment`) | 0 | 0 |

`exportsProven` stays 0: closing `creates` alone closes no export.

## Checks run

- Go: the producer's package suite (`apps/solid-typefacts`), including the new
  subject-parameter premise table and the `.call`/`.apply` receiver test.
- Rust: `solid-facts-backend --lib` armed with the build's own pins (387 passed),
  the `typefacts` and `solid-reactive-ir` library suites, the process suites,
  workspace Clippy with `-D warnings`, `cargo fmt --check`.
- Gates: coverage (94 projects, 547 findings, no movement), contract corpus
  non-updating after the reviewed update, dialect manifests, schema `jq`,
  `git diff --check`, the phase 19 audit.

`make verify`, `make ecosystem-benchmark` and every baseline repin were
deliberately not run for this change.
