# ADR 0042: The iteration protocol, by whose value is iterated

- Status: accepted and implemented (2026-09-06); written with the
  implementation
- Date: 2026-09-06
- Owners: Type Facts producer (`uncensused_invoking_forms.go`,
  `export_value_transcripts.go`) and the policy-2 `creates` census
  (`type_facts.rs`)
- Relation: the fourth slice of ADR 0034's premise, after ADRs 0040 and 0041.
  Resolves one of ADR 0038's named remaining refusals. Handshake protocol
  25 → 26; lever G in `docs/package-contract-v2/accuracy-roadmap.md`.

## Context

Until now the census asked one question of an iteration: *does the operand's
type name a container whose iterator is the engine's?* For compiled JavaScript
the answer is almost always no, because an unannotated parameter is `any` and
`any` enumerates no members at all. That left 90 refusals at 15 sites, the
second-largest form class after the accessors.

One shape dominates it. Five packages in the corpus publish this helper
verbatim — `@corvu/utils`, `@corvu-next/utils`, `@solid-primitives/utils`,
`solid-transition-group` and their siblings — and it accounts for 58 of the 90:

```js
var chain = (callbacks) => {
  return (...args) => {
    for (const callback of callbacks) callback && callback(...args);
  };
};
```

Three separate refusals meet in those two lines, and none of them is about code
this module wrote:

- the `for…of` drives `Symbol.iterator` on `callbacks`, the caller's iterable;
- the `...args` spread drives iteration on `args`, a **rest parameter**;
- `callback` is a value that iterable yielded, and calling it runs the caller's
  function.

## Decision

**The iteration protocol is dispositioned by the provenance of the value
iterated, not by its type**, in three parts.

### The caller's iterable

An `iteration-protocol` form — a `for…of`, a spread element, or an array
binding pattern — whose iterated value the producer roots at an unwritten
parameter of the censused declaration is dispositioned
`parameter-rooted-iterable`. The `Symbol.iterator` it reaches, the `next` calls
that follow and any `return` on early exit all sit on the object the caller
passed, so the code that runs is the caller's exactly as a getter's is
(ADR 0034), and this export's act is the iteration. The subject is named the
same way ADR 0041 names a spread's and a pattern's: the operand for a `for…of`
and a spread element, and what the outermost enclosing pattern destructures for
an array pattern.

**`for await…of` states nothing.** It drives `Symbol.asyncIterator` and the
promise machinery that awaits each result, a reach no ADR has reviewed, and it
refuses however it is rooted. Pinned by `awaitIterateParameter`.

### The engine's array

A rest parameter's binding records **no form at all**. The specification builds
that array with ArrayCreate at every call, so it is an ordinary `Array`, never
a Proxy and never an object a caller shaped, and iterating or spreading it
reaches `Array.prototype[Symbol.iterator]` and nothing else — the standing the
census already gives every default-library member, not a new premise about the
caller.

This is a *syntactic* fact and the type cannot supply it: an untyped rest
parameter is `any[]`, and an `any[]`-typed value need not be an array. The
binding must be unwritten, for the reason ADR 0034 gives.

### The value the iterable yielded

A call may state `calleeIteratedParameter`: the parameter-rooted iterable whose
iteration produced the callee. The census dispositions such a call
`parameter-rooted-element` — what the caller's iterable yielded is the
caller's, exactly as a callee that *is* a parameter is (ADR 0008 § 4.4), and
the `callbacks` domain owns it.

The producer states it only for a plain, non-`await` `for…of` whose head
declares exactly this one binding, with no initializer of its own, which
nothing in its file writes, over an expression rooted at a parameter by the
same walk `calleeParameter` uses. Nothing in the census re-derives it.

## What this resolves

ADR 0038 named this refusal and left it standing: *"a structural iterable
(`Iterable<number>`) as a spread or `for…of` operand: its iterator is the
caller's. Pinned by the census fixture's `spreadUntyped`."* It was right about
the fact and stopped at the type question. Asking provenance instead resolves
it, and `spreadUntyped` now certifies.

## Alternatives considered

- **Widen the engine-owned-iterator type table** to admit `any` or a structural
  `Iterable`. Rejected, and it is the reading ADR 0026 exists to forbid: "the
  checker could not find `[Symbol.iterator]`" is never "iterating this reaches
  no user code".
- **Treat a rest parameter as a subject root** so it flows through the ADR 0034
  machinery. Rejected: ADR 0034 excludes rest parameters precisely because the
  array is *not* the caller's object, which is the fact that makes iterating it
  safe here. Conflating the two would make the receipt say the caller supplied
  something the engine built.
- **Disposition `for await…of` under the same premise.** Rejected: the async
  protocol reaches more than the iterator, and no ADR has reviewed that reach.
- **Follow the loop variable through further assignments.** Rejected: the
  binding must be unwritten, as everywhere else in this family.

## What still refuses

- **An iteration of anything not parameter-rooted** — a module value, a call
  result, a local. Pinned by `chainModuleCallbacks`.
- **`for await…of`**, however rooted. Pinned by `awaitIterateParameter`.
- **A callee bound by anything but a plain `for…of` head** over a
  parameter-rooted iterable: a `for…in`, a destructuring, a written loop
  variable, a head declaring more than one binding.
- **A written rest parameter**, and a rest parameter in a declaration
  mentioning `arguments` or `eval`.
- Every other form ADR 0034 left refusing.

## Consequences

- Handshake protocol 25 → 26; the schema digest and the Rust client move with
  it.
- `implementation-census-creates` gains `chainCallbacks` (certifies, the whole
  `chain` shape), `chainModuleCallbacks` and `awaitIterateParameter` (refuse),
  and `spreadUntyped` flips from refusing to certifying.
- Measured on the 2026-09-06 corpus: withheld candidates 739 → 675,
  `censusRefused` 682 → 618, and the iteration class 90 → 18 refusals at 15 →
  9 sites. **Sixty-four candidates**, the largest gain of this lever, and
  almost all of it survives as closure rather than moving on: the accessor
  class rises only 277 → 285, because `chain` is a leaf whose body is the loop
  and the call and nothing else. Statuses unchanged, 368 certified / 30
  refused.
- Tests: `creates_census_dispositions_a_parameter_rooted_iteration` pins all
  three node kinds, the unrooted refusal, and both call cases;
  `the_probe_gate_tracer_census_closes_a_parameter_rooted_spread` replaces the
  tracer test that pinned `spreadUntyped`'s refusal.
