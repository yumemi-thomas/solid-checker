# ADR 0109: A `returns` census for a merged props root

- Status: accepted and implemented (2026-09-15); written before implementation
- Date: 2026-09-15
- Owners: the policy-2 behavioral call-domain census, the semantic model's
  return shapes and the stable-v1 schema, the generator's return derivation,
  and the rules engine's props-root classification
- Relation: the third decidable shape of ADR 0035's `returns` census, beside its
  empty closure and ADR 0075's exhaustive whole-parameter identity. It consumes
  the dialect row `Dialect::merges_props_reactivity` (2026-09-15). It does not
  touch `reads`, `callbacks` or `creates`, and it needs **no producer change**.

## Context

`@kobalte/utils` exports

```ts
export function mergeDefaultProps(defaults, props) {
  return mergeProps(defaults, props);
}
```

and `kobalte/packages/core` calls it 254 times. 127 open-claims findings on that
export ask for `reactiveReads` and `returns`
(`phase21/2026-09-14-which-closures-change-a-consumer-finding.md` § 23). Every
props-forwarding wrapper in the ecosystem has this shape; `@solidjs/meta`'s
`Stylesheet` and `@solidjs/router`'s `A` and `Route` are the same function.

Three things had to be measured before this ADR could be aimed anywhere.

**The gate wants a closed domain, not a stated claim.** `project_callbacks` and
its siblings open a domain whenever ingested knowledge is `!is_closed()`, and
`push_unknown_contract_claims` reports exactly that. A contract full of true
statements still produces the finding. This is why the 2026-09-14 re-export work
moved zero findings while publishing a correct contract.

**`reactiveReads` was never this domain's problem.** A clean wrapper of the
identical shape closes `callbacks`, `reads` and `creates` today —
`fixtures/package-contracts/callback-slot-props-forwarding`'s `Stylesheet`. What
left the real `@kobalte/utils` publishing `closed: ["creates"]` alone was the
attribution ladder widening an unrelated obligation to every export, fixed
2026-09-15. So of the two domains the 127 demand, one is already answered and
`returns` is the whole remainder.

**`returns` is walled twice, and neither wall is the producer.**

1. *The claim cannot be written.* `validate_contract_return` refuses a
   `parameter` on a reactive leaf — "a reactive leaf requires a label" — and on
   the wire the semantic model has `ValueShape::Store { resource, capabilities }`
   with no root and `ValueShape::Parameter { index, path }` with no reactivity.
   "A props object whose reads reach through to argument 1" has no spelling.
2. *The claim cannot be proved.* `census_returns_domain` decides the empty
   enumeration and one whole-parameter return, and refuses everything else by
   name: "a returns closure candidate must enumerate no operation or one
   whole-parameter return".

The producer, by contrast, already states every fact this census needs, at the
current handshake protocol (59) and well below it:

- `ReturnSite.sources` carries `ImplementationValueSourceKind::CallResult` with
  `target_name` and `target_module`, and its own doc fixes the reading: "a
  consumer that needs dialect identity must ask the dialect whether that
  specifier exports that name". The certifier already reads exactly this shape
  for the factory-return premise, filtering `target_module == "solid-js"` and
  deferring to `solid_dialect`.
- `ImplementationCall.argument_parameters` gives, per written argument slot, the
  parameter of the censused declaration that slot is rooted at.
- `ReturnSite.reach`, `completionForm` and the classified control-flow census are
  ADR 0035's existing premises.

So this is not a producer ADR. It is a model-and-census ADR, and the schema is
the expensive part.

## Decision

### The claim this census decides

One new return shape, meaning:

> One invocation of this export yields an object whose property reads reach
> through to the caller's argument at index *N*.

It is **parameter-relative and conditional on purpose.** `mergeProps` creates no
reactive source; it carries its arguments'. `solid-js@1.9.14`'s `dist/solid.js`
returns a `$PROXY` only when some source is already a proxy or is a function
(memoised on the way in), and otherwise rebuilds the object preserving each
source's own descriptors. `mergeProps({ a: 1 }, { b: 2 })` is therefore plain,
and destructuring it loses nothing. A claim that said "this returns a store"
unconditionally would manufacture a finding on that call — which is precisely
the mistake the 2026-09-14 demand report § 18 item 4 proposed and the
2026-09-15 backlog entry falsified. The claim has to name the argument, and the
consumer has to look at what it passed.

### What the census requires

The census certifies the closure when, on the demanded export's own
`ExportImplementationTranscript`, every premise below holds. Each failing
premise refuses **by name and location**; none is a silent pass.

1. **Plain completion form.** ADR 0035 premise 1 unchanged: neither `async` nor
   a generator, and `unclassified` refuses. A promise or an iterator is not a
   props object.
2. **Every reachable completion is the same merge.** For each `ReturnSite` at
   the `MayExecute` floor, either its reach is `unreachable` (admitted with a
   witness naming it, as in ADR 0035 premise 3), or it carries a value whose
   `sources` contain a `CallResult` whose `target_module` the dialect owns and
   whose `target_name` the dialect's `merges_props_reactivity` row names. An
   empty `sources` list is the producer's silence and refuses — it is never
   "traced to nothing, therefore plain". ADR 0035's block-body premise is
   **dropped** here: an expression-bodied arrow's completion is a `ReturnSite`
   like any other, and here the value-carrying return *is* the claim.
3. **One parameter-rooted merge source, and it is the one claimed.** The merge
   call named by the source is located in this transcript's own `calls`; exactly
   one of its `argument_parameters` is `Some`, that one has an empty path, and
   its `parameter_index` is the *N* the proposal names. Two parameter-rooted
   sources are a claim about two arguments this shape cannot spell, and refuse
   rather than pick.
4. **Every other merge source is inert.** A source that is provably callable
   makes the result reactive whatever the caller passed, which would make the
   conditional claim false in the dangerous direction — the consumer would read
   "safe unless you passed a reactive argument" about an object that is always
   reactive. So every argument other than *N* must be an own literal of this
   implementation (the existing own-literal subject), and anything else refuses.
5. **Control flow is classified and admissible, and the transcript is
   complete.** ADR 0035 premises 4 and 5 unchanged.

Multiple return sites are admitted when every one of them satisfies 2–4 with the
same *N*. Nested callables are not a premise, for ADR 0035's reason: a `return`
inside one is that function's completion.

### What must be built, in dependency order

**1. The model and the wire.** A new `ValueShape` variant carrying the root —
sketch `MergedProps { from: u16 }` — with its `ContractReturn` projection, its
`normalize_export` round trip, `validate_contract_return`, and
`schema/solid-reactivity.schema.json`.

*This is the ADR's one genuinely open decision.* A new variant is **not**
transparently backward compatible: the wire enum is externally tagged with
`deny_unknown_fields`, so an older verifier refuses the whole document rather
than ignoring the shape. The alternative — an optional `root` field on the
existing `Store` shape — is worse, and the reason is the point of the whole ADR:
an older decoder that ignores `root` reads the return as an *unconditional*
store, which is a **stronger** claim than the truth, on exactly the
`mergeProps({a:1},{b:2})` case. Silent strengthening is the failure mode the
precision contract exists to prevent, so a loud refusal is the right trade — but
it is a compatibility break to schedule deliberately, not a footnote. It should
land with the schema's compatibility rule restated and with whatever the stable
cut's versioning policy says about a new shape.

**2. The census.** A third arm in `census_returns_domain`, beside the empty
enumeration and `census_parameter_returns_transcript`. It reads the transcript
only — no callee disposition, no recursion into local declarations, no accepted
dependency — for ADR 0035's reason: a callee's value reaches this export's
caller only through this export's own completion.

**3. The proposal side.** `Returns` is already in `ClaimDomain::PROPOSABLE`. The
generator derives the shape from its own syntax facts: a function export whose
every completion is a call of a primitive `merges_props_reactivity` names, with
exactly one parameter-rooted argument and every other argument an object
literal. A proposal input and never a proof; silence is "do not propose", so no
export that certifies today can lose that certification to this candidate.

**4. The consumer.** `binding_initializes_reactive_store` and the member-read
classification learn one conditional: on reading this shape from a contract,
ask whether *this call site's* argument at *N* is itself a props root, a store,
or a function. Yes makes the binding a props root; no or unknown makes no claim.
This is the same conditional `discover_sources` already applies to a direct
`mergeProps` call through `Dialect::merges_props_reactivity`; the contract path
extends it through one wrapper.

**5. The veto.** Recipe-gated under the mandatory veto like every proposed
closure. The natural shape: call the export with a getter-bearing object at *N*,
read a property off the result, and emit the marker if the getter never ran. A
finite clean run never establishes the closure; the census does.

## Alternatives considered

- **Reuse `store-path` unconditionally.** What the 2026-09-14 report proposed.
  Falsified against the audited runtime: it manufactures a finding on
  `mergeProps({a:1},{b:2})`, and on the destructuring of any merge of plain
  objects. Not taken.
- **Reuse the existing `argument` kind**, which already carries a parameter. It
  means "yields that argument itself", which is false of a merged object and
  which the whole-parameter census would refuse on sight — the return traces to
  a call, not to the parameter. Not taken.
- **Let the consumer walk the dependency's bytes** and decide for itself. A
  parent's census must never walk another archive's bytes; that refusal is
  correct and is quoted in `phase21/2026-09-15-closure-gap-plan.md` § 1. Not
  taken.
- **Close `returns` as empty by treating a props object as "not a value".** The
  semantic model's 2026-09-03 decision is about *valueless completion*, and a
  merged object is a value. Not taken.
- **Do nothing.** Defensible on cost, and the 127 are the checker honestly
  admitting ignorance rather than a false claim. But the ignorance is now
  avoidable: every producer fact exists, the dialect row exists, and the
  consumer conditional exists for the direct call. What is missing is a spelling
  and a census arm. Recorded as the reason this ADR is written before it is
  scheduled.

## Implementation (2026-09-15)

- **Producer.** Unchanged, as predicted. No new fact, no protocol bump, no
  schema digest move.
- **Model and wire.** `ValueShape::MergedProps { from }`, canonical discriminant
  **17** — appended, never inserted, because a discriminant is part of the
  semantic digest and renumbering an existing shape would move every receipt
  that ever described one. `WireValueNode::MergedProps { from }` with `from`
  **required**: an absent index states nothing, and defaulting it to zero would
  name a parameter the producer never claimed. `mergedPropsValue` in
  `schema/solid-reactivity.schema.json`. `ContractReturn` carries it as
  `kind: "merged-props"` with a `parameter` and no label, which is why
  `validate_contract_return` files it with the parameter-carrying kinds rather
  than the reactive leaves.
- **Census.** `census_merged_props_returns_transcript`, dispatched from
  `census_returns_domain` after the whole-parameter arm — the narrower claim
  first, since a return that *is* the parameter is not a merge of it. ADR 0035's
  premises 1, 4 and 5 are now shared through
  `require_plain_classified_completion` rather than restated, so the two arms
  cannot drift. `solid_dialect::unambiguous_props_merge` is the dialect-agnostic
  query the certifier needs: a transcript names a module specifier and an export
  and never says which dialect the artifact resolved, so the answer is taken
  only where both dialects agree — which, since each is silent about the other's
  spelling, means `mergeProps` is decided by 1.x alone and `merge` by 2.0 alone.
- **Generator.** `returns_walk::merged_props_return` beside
  `valueless_completion`, collected project-wide into
  `Program::merged_props_returns` (off the wire, fail-closed `Default`) because
  it must resolve a callee to a dialect primitive and that needs the entity
  tables. Attached at the emit boundary as `ContractExport::merged_props_return`
  by the same two identities the other two walk verdicts use. In
  `normalize_export` it is matched **before** the empty closure, which is
  documentation rather than necessity: a body that returns a merge yields a
  value, so the valueless-completion walk declines on the very return this one
  reads.
- **Consumer.** One arm in `binding_initializes_reactive_store`. It is the first
  contract claim whose meaning depends on the caller's argument, and it answers
  `false` rather than `unknown` for a plain argument — the merge of two plain
  objects is plain, and destructuring it loses nothing.
- **Measured.** `callback-slot-props-forwarding` moves and nothing else does:
  `Stylesheet`, `WithDefaults` and `WithLazyExtras` gain
  `returns: [{merged-props, from 0}]` and close `returns`, while `makeStore`,
  `makeSignal` and `derive` are untouched. Coverage stays at 550 findings across
  94 projects, because no fixture consumes an accepted contract — see
  Consequences. Seven census cases are pinned by
  `merged_props_returns_census_binds_every_completion_to_one_parameter`.

### Deviation from the decision above

**Premise 4 is dropped**, and the ADR was wrong to require it. It asked that
every merge source other than the claimed parameter be an own literal, to stop a
consumer reading "reactive only if you passed a reactive argument" about an
object that is always reactive. Two things are wrong with that. The error it
prevents is an **under**-report — the consumer misses a finding — which is the
safe direction and not what a premise is for. And no producer fact proves an
object literal inert: the argument tracer leaves an object literal's slot empty,
which is indistinguishable from untraced, so the premise could only have been
implemented as "refuse everything", making the census decide nothing.

What survives is the half that is about this shape's *expressiveness* rather
than about safety: two whole-parameter sources are a claim `merged-props` cannot
spell, and choosing between them is not a census decision, so they refuse. The
census is therefore sound but not complete — it certifies "reads reach through
to argument `from`" and says nothing about whether they also reach elsewhere.

## Consequences

- **Producer.** None. No new fact, no protocol bump, no schema digest move —
  the first behavioral-domain census in this series with that property.
- **Schema.** One new value shape, and a deliberate compatibility break for
  verifiers below it. This is the gating cost and the reason to schedule rather
  than slip it in.
- **Certifier.** A third arm in `census_returns_domain`; new `census-return:`
  witness sites naming the merge, the parameter and each admitted site.
- **Generator.** A merged-props return walk beside the valueless-completion
  walk. Corpus snapshots move for every props wrapper the walk clears; review
  the non-updating run first.
- **Consumer.** One conditional. It is also the first contract-mediated claim
  whose meaning depends on the *caller's* argument, which is worth naming: every
  existing return shape is absolute.
- **Fixtures.** The generation half is authorable today
  (`callback-slot-props-forwarding`'s `Stylesheet` is already the shape). The
  consumer half was **not**, and is now: the blocker this bullet recorded — every
  fixture-supplied catalog being `obsolete-policy1`, because an accepted one
  needs a receipt a fixture cannot forge — was closed the same day by
  `src/fixture_authorization.rs` and `scripts/coverage.mjs`, which authorize a
  fixture's own resolution in a scratch copy and supply the trust out of band.

  **Measured, and the arm is exactly as narrow as this ADR claims.**
  `fixtures/reactive-ir/package-merged-props-consumer` calls two exports of the
  same shape four times, varying only where the props sit: `withDefaults(props)`
  (contract names argument 0) and `withOverrides({…}, props)` (names argument 1)
  report `SC1003`; `withDefaults({label: "fallback"})` is clean, which is the
  conditional answering `false` rather than `unknown`. The fourth,
  `withOverrides(props, {…})`, is also clean and is this ADR's admitted
  incompleteness in a snapshot: a real merge carries argument 0's reactivity and
  the census does not certify it.
- **Yield, honestly.** It closes `returns` for props wrappers. Whether the 127
  *disappear* depends on `reactiveReads` also closing, which the 2026-09-15
  attribution fix should deliver — but neither half is measurable in this
  repository, which has no kobalte corpus and no pass-2 harness. The prediction
  to falsify when someone runs it: `mergeDefaultProps`'s findings go to zero,
  and no finding appears on a merge of plain object literals.
- **Not changed.** `reads`, `writes`, `callbacks`, `creates`, `cleanups`,
  `disposals`, `invalidates`, `throws`; ADR 0035's empty closure and ADR 0075's
  whole-parameter identity, both of which keep their premises exactly.
