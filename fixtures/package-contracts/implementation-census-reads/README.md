# `implementation-census-reads`

The tracer for the `reads` implementation census, and the home of the first
hand-authored `reads` probe recipe.

## The claim under test

`semantic-model.md` § reads **[Decision 2026-09-10]**: a property access on a
store, props, or projection proxy is *this export's* read only when the proxy
is a value the export **owns**. A read reached through a caller-supplied value
is the caller's, on [ADR 0034](../../../docs/adr/0034-parameter-rooted-accessor-disposition.md)'s
argument about whose code runs.

So these exports split on the receiver's **provenance**, not on syntax. Every
one is a property access or an invocation; what differs is whose value is
underneath.

| entrypoint | export | receiver | `reads: []` should |
| --- | --- | --- | --- |
| `.` | `plainArithmetic` | — | close |
| `.` | `readsOwnLiteral` | own object literal (ADR 0044: data properties) | close |
| `.` | `readsCallerMember` | caller's object, `.value` | close |
| `.` | `readsCallerElement` | caller's object, `[key]` | close |
| `.` | `invokesCallerAccessor` | caller's callable — the export's act is the invocation (census plan § 3.2) | close |
| `./owned` | `readsOwnProxy` | a proxy **this module built** | **refuse** |
| `./owned` | `readsOwnProxyElement` | the same, element access | **refuse** |
| `./owned` | `observedReads` | — | **refuse**, and that is the point |

`observedReads` reports how many times `ownProxy`'s trap ran, so a recipe can
watch this module's own source. It touches no proxy itself and still refuses,
because the refusal is a fact about a **closure**, not an export.

## Two entrypoints, and why the split is the design

The premise the census cannot obtain — "no accessor installed at run time
reaches this read" — can only be attributed to the file that carries the
installation. TypeScript types a `Proxy` as its target, so a read through one
records no invoking form for any census to refuse
([the design](../../../docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md)
§ 6-§ 10), and the syntactic hazard is the only place it stays visible.

So a single `new Proxy` anywhere in a file withdraws `reads` for every export
in it. The proxy therefore lives in `./owned`, with its own closure. Splitting
per *export* would need dataflow from the installing expression to each read's
receiver, which nothing in this pipeline has.

The rows above are the current behavior, not an intent. `reads` is in
`ClaimDomain::PROPOSABLE`, the census decides it, and
`a_reads_closure_reaches_a_receipt_through_its_mandatory_veto` carries
`plainArithmetic`'s closure through its gate to a policy-2 receipt using
`probe-recipes/plain-arithmetic.mjs`.

`probe-recipes/recipes.json` is **not** what that test reads: it plans the
package itself and asks the schedule for the live claim id, because the ids
are content digests and the split moved every one of them. The manifest is
kept as the hand-authored record of which module addresses which claim; treat
its ids as stale until a run reprints them.

## No `solid-js` import, deliberately

The census cannot prove a receiver is not a proxy, so a real Solid store would
add nothing this fixture does not already state, and would make every row
depend on an accepted-dependency closure the way
`implementation-census-creates`' README warns about. `ownProxy` is a bare
`Proxy`, which is the same thing to the census: a value built by a *call*,
whose subject root `census_form_disposition` does not clear.

## Measured state (2026-09-10), and what is still open

The generator proposes `reads: []` for **all eight** exports, including
`readsOwnProxy` and `readsOwnProxyElement`, and the plan carries all eight as
closure candidates with claim ids. That is the designed flow — a proposal is
the generator's inference, and the certifier's census must re-prove it — but
**whether the census refuses the two proxy rows has not been observed here**:
the corpus gate stops at the proposal and plan, and certification needs the
registry path. Until it is observed, this fixture states the intent and pins
the candidates; it does not yet prove the refusal.

## The recipes

`probe-recipes/` holds the first `reads` recipes in the repository.

- **`plain-arithmetic.mjs`** — runs the export, observes no read, emits
  nothing. The closure stands because nothing contradicted it.
- **`reads-own-proxy.mjs`** — runs the export and emits `read-operation`,
  because the read really happens. It also asserts its *own* observation
  fired, so a fixture edit that removes the trap fails the recipe instead of
  silently passing the gate.

`recipes.json` pins both to the claim ids in `expected-proposal.json`; the two
files move together, and a regenerated plan means regenerated claim ids.

**Why hand-authored.** The observation is exact for this package precisely
because its author knows that `ownProxy` is every reactive-shaped source the
module owns. A synthesized veto cannot know that — it can only instrument
values the *caller* supplied, which is the half § reads assigns to the caller.
That is the argument in `phase21/2026-09-10-reads-veto-observation-design.md`
for registering no synthesized observation for the domain, and this fixture is
its worked example.
