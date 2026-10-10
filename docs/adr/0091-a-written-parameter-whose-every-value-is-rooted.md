# ADR 0091: A written parameter whose every value is rooted

- Status: accepted and implemented (2026-09-11); written with the
  implementation
- Date: 2026-09-11
- Owners: Type Facts producer (`uncensused_invoking_forms.go`)
- Relation: ADR 0050's argument, applied to the binding it excluded. Handshake
  protocol 48 → 49.

## Context

ADR 0050 rooted a **local binding** the file writes, when every value it can
hold is rooted, and said why the restriction it lifted was never about flow
sensitivity:

> if every value the binding can hold is the caller's, then whichever one it
> holds at the read is the caller's, and which branch assigned it never comes
> up.

It left the same restriction standing on *parameters*.
`parameterSubjectRootsLocked` drops a written parameter outright, so the
commonest compiled shape in the corpus still refused:

```js
if (typeof b === "string") { b = stringStyleToObject(b); }
return { ...a, ...b };
```

Protocol 48's `subjectRootRefusal` measured what that costs:
**30 distinct claims across 18 packages** state `written-parameter`, the second
widest leg of the accessor class and the widest one with a premise available.

## Decision

A parameter roots at its own slot under the unchanged `parameter` derivation
when

- its binding is a plain identifier, not a rest parameter, carrying no default;
- the body writes it; and
- **every** value assigned to it is rooted at that same slot.

The sources are the right-hand side of every plain `=` assignment within the
censused declaration. The parameter's own slot is not among them: it is the
caller's argument by construction, and that is the provenance in question. A
self-reference is admitted co-inductively, which is the chain rule applied to a
loop.

## What refuses, and why each is a refusal rather than a skip

- **A compound assignment, an update expression, a destructuring target, a
  `for…of`/`for…in` head** — the whole binding refuses. A skipped write is a
  value nobody enumerated, and this premise is a claim about *every* value.
- **Two different slots.** The value is the caller's either way, but the
  receipt names one slot and naming either would say the caller passed
  something it did not — ADR 0050's boundary verbatim.
- **A value this module made, or the result of a call this build does not
  premise.** The join holds only when every source is the caller's.
- **A binding the write predicate calls written for which no source was
  enumerated.** The two predicates disagreeing is the one way this join could
  be unsound — it would root a binding whose every write went unseen — so an
  empty source list refuses rather than being read as "nothing is assigned".
  `writtenParameterDestructured` pins it.

## Consequences

Handshake 48 → 49 although the spelling is unchanged: a consumer that reviewed
only the unwritten reading must refuse this one rather than read it as the
weaker claim, which is ADR 0043's rule.

Measured on the 2026-09-11 corpus — see
`docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`
§ 61.
