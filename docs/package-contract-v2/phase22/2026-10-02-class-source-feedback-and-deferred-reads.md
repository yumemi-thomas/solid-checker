# Class source feedback and deferred reads

The new source experiment explains six of nine newly introduced collection
snapshot mistakes before execution. It follows exact installed class exports,
private fields and method declarations into an imported dependency. It needs no
package-specific rule or authored contract.

These results are **informational feedback, not proven violations**. An
intentional snapshot produces the same source note. Also, calling `getObserver`
does not by itself prove that a returned value is reactive. A focused synthetic
case checks an observer and returns the constant `42`; it has the same positive
source footprint. Publishing these footprints as reactive contracts would repeat
the earlier closed-world summary mistake.

## New consumers and actual execution

Twenty consumers run with unchanged packages and with the existing bounded guard
trace: **40 fresh browser executions**, all passing strict consumer checks against
the real published typings. Observed values, captured errors and diagnostic
feedback agree between the two profiles. All direct semantic diagnostic channels
are quiet on these cases.

| Introduced mistake | Source note | Browser guard note | Update expectation fails |
| --- | --- | --- | --- |
| Map `get` captured during setup | yes | yes | yes |
| Map `size` captured during setup | yes | yes | yes |
| Set `has` captured during setup | yes | yes | yes |
| WeakMap `get` captured during setup | yes | yes | yes |
| WeakSet `has` captured during setup | yes | yes | yes |
| Namespace-imported Map `get` captured during setup | yes | yes | yes |
| Computed `map['get']` call captured during setup | open | yes | yes |
| Iterator consumed during setup | open | yes | yes |
| Read from an instance that escapes through an alias | open | yes | yes |

Ten of eleven valid controls have no source note. The remaining control is an
intentional, unmarked snapshot. Its app bytes are identical to the incorrect Map
`get` case; only the externally declared desired value differs. Promoting every
note to a defect warning would therefore create a known false positive.

The explicit `untrack(() => map.get('key'))` snapshot stays quiet in the bounded
source pass. It still produces a runtime guard note, because the observer really
is absent. This is evidence that a declared sampling convention can help, not a
completed intent-aware feedback implementation. The prototype does not deliver
project-policy warnings or establish that every use of `untrack` is intentional
sampling.

Method replacement, a shadowed constructor import and a deferred read callback
also remain quiet. A replaced method and an escaping instance are explicitly
refused rather than interpreted using the original class body.

## Timing must cross the source boundary correctly

`map.keys()` creates a generator. The generator's tracking call executes when
the iterator advances. Walking its body in the caller's execution phase would
manufacture an eager read. The extractor refuses generator and async bodies;
it also refuses immediate summaries for methods that delegate to a generator.
The actual setup-time iterator-consumption mistake consequently remains a miss.
Supporting it requires an execution fact for iteration, not more export names.

Callbacks stored or passed to an unknown helper are not walked as immediate
calls. Public-field dispatch, computed members, ambiguous exports, recursive
paths beyond the budget, and private fields reassigned in the class remain open.
Source methods replaced through `this` are refused too. External prototype
mutation and reflective behavior are not closed by this experiment.

The source observation is deliberately modest: an exact core observer call
occurs in a non-deferred source body reached through bounded declaration links.
It is not a proof of branch reachability, subscriber installation, output
reactivity, or complete behavior. No negative domain is inferred from an empty
footprint.

## Coverage and cost

The same source pass inspects the earlier 50-consumer corpus. It adds notes for
the Map and Set targets and none for its 26 controls. Forty-eight consumers enter
the rc.9 source vocabulary; the query pair on rc.4 remains refused. Existing
published app asset declarations are retained in the typing program. No new
native analyzer findings are claimed: the earlier **4/23** static detection
result and **15/24** direct runtime result are unchanged. The two additional
source notes are a separate feedback category.

Across 97 retained primitives package roots, the inventory resolves nine class
exports. Five expose bounded immediate footprints, comprising nine methods and
two getters across **Map, Set and Trigger**. Three roots still lack their shipped
entry: animation, controlled-props and virtual. This class mechanism alone has
limited package reach. It does not cover store getters, proxies, dynamic property
installation, callback phase, component providers or asynchronous lifetimes.

For the twenty new consumers, source extraction and flow matching take **23.1 ms
median / 26.9 ms p90** on this machine, including local closure checks. These
measurements start after the TypeScript program is built and exclude browser
execution and native analysis. They are not an editor latency measurement or a
warm-cache benchmark.

## Implementation and evidence

The isolated files are under `benchmarks/reviewed-package-models/`:

- `class-footprints.mjs`: bounded class paths and setup-to-JSX flow observations;
- `class-footprint-cases.mjs`: nine targets and eleven controls;
- `class-footprint-study.mjs`: fresh published typing and source observations;
- `class-footprint-inventory.mjs`: the 97-root source inventory;
- `validate-class-footprints.mjs`: byte identities, actual typing, intent witness
  and finite execution parity;
- `class-footprints.test.mjs`: exact imports, shadowing, mutable receivers,
  generator timing, unknown callbacks and the nonreactive observer counterexample.

Ignored evidence lives under `rust/target/`:

- `class-footprint-browser-original/results.json` and
  `class-footprint-browser/results.json`;
- `class-footprint-study-verified.json`;
- `class-footprint-inventory-handoff.json`;
- `class-footprint-validated-handoff.json`.

The validator checks the frozen extractor and consumer bytes, installed closures,
source premise hashes, original consumer locations, all 40 new executions and
the identical-code intent witness. The source study contains 70 observations,
of which two are explicitly refused for the older query runtime.

All **36 prototype tests pass**, including six new class tests. Syntax,
whitespace and universal handoff checks pass. Full `make verify`, native fixture
coverage, ownership gates and contract certification are deferred: production
semantics, public contracts, schemas and fixture snapshots are unchanged. Only
experimental outputs and investigation documentation are added.

## What this changes in the decision

Source-derived feedback can fill some quiet runtime gaps cheaply, including
transitive behavior hidden behind class methods. A growing collection of
positive source patterns still cannot supply most-package coverage by itself.
The broad path remains shared runtime diagnostics plus targeted source analysis
and update/lifetime expectations. Automatic defect feedback requires stronger
execution facts; snapshot intent requires an explicit expectation or an adopted
project convention. Neither should be replaced with an assumed package contract.
