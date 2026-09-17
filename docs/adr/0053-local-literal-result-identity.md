# ADR 0053: A local helper returns one data-only allocation

Status: implemented; full corpus confirms six closures. Full `make verify`
passed with exit 0, TOTAL 231.78 seconds, and no failure marker.

## Premise

A property access can use the own-literal premise after an ordinary local
call when every normal completion of that exact callee returns the same
unwritten, locally allocated data-only literal binding. This is source
identity, not a structural return type. It reuses ADR 0044's stated property
mutation assumption; it does not claim an escape analysis or protection
against arbitrary accessor/prototype installation by third parties.

Protocol 36 carries `localLiteralResult` on the recorded form, with the exact
call, callee, allocation and complete list of return-site locations. A
terminal return explicitly covers fallthrough. Every return must name the
same binding, initialized with a data-only literal in this callee, and that
binding must be unwritten. Async and generator completion wrappers refuse.
Nested callable returns do not belong to the enclosing completion set.

At the use site only a direct call result or a chain of unwritten local
aliases preserves the result's identity. A property held in the result is
an arbitrary value and does not inherit the premise. The producer maps every
location from a premise twin back to original source coordinates.

## Consumer and receipt

The consumer defers the access until the ordinary call walk has demanded and
censused the exact callee under that call's argument premises. It matches a
unique call row, resolves a stable local declaration in authenticated runtime
source, obtains that callee's own transcript, and requires the premise's
callee location to match it exactly. The allocation and nonempty return set
must lie within that callee. Contradictory subject derivations refuse.

The witness names the complete derivation as `census-local-literal-result:`
and distinguishes `local-literal-result-accessor` from its write-position
variant. The Type Facts transcript and source roots bind the producer's
complete identity statement, while the execution census binds every invoked
operation. This result premise grants no iteration, coercion or callable
identity by itself.

## Evidence and remaining boundary

The producer regression confirms a recorded form for each case, covering
same-file and cross-file aliases, replaced and mixed returns, fallthrough,
accessor-bearing literals, async/generator completions, module allocations,
rewritten result bindings and nested property reads.

The packed native fixture runs the live producer, census, synthesized veto
and receipt finalizer. Its complete-return case closes in both read and write
positions. Mixed, fallthrough, accessor and replacement controls remain at
the recorded form. Consumer tests substitute call, callee, allocation and
return locations, remove returns or the helper transcript, and introduce a
contradictory parameter derivation; each must refuse without a witness.

This first slice targets the SVG scrape result: the HTML helper returns its
`newValues = {}` allocation on both paths. Recursive/mixed result shapes,
arrays produced by library calls, host selector results and dynamic parsers
need separate premises; none is assumed closed here.

The [full measurement](../package-contract-v2/phase21/2026-09-07-literal-result-census-measurement.md)
confirms all six SVG candidate occurrences close: withheld 468 → 462, creates
census 411 → 405, rows unchanged at 368 / 30. All six accepted mains state
closed empty creates and bind to their receipts. No other remaining reason
changes; the residual B families require a different completion derivation.
