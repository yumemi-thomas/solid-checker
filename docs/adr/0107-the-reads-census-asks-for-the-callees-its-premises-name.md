# ADR 0107: The `reads` census asks for the callees its premises name

- Status: accepted and implemented (2026-09-14); written with the
  implementation
- Date: 2026-09-14
- Owners: the `reads` implementation census
  (`contract_certification/type_facts.rs`)
- Relation: ADR 0093's derivation, made reachable from the domain that needed
  it. **No handshake protocol change** — the producer already states this fact.

## Context

ADR 0093 roots a written parameter holding a value this program allocated, under
the derivation `parameter-or-own-result`, and named its motivating shape:

```js
function stringStyleToObject(style) {
  const object = {};
  while (match = re.exec(style)) { object[match[1]] = match[2]; }
  return object;
}

function combineStyle(a, b) {
  if (typeof b === "string") { b = stringStyleToObject(b); }
  return { ...a, ...b };            // reads own properties of b
}
```

That ADR landed on 2026-09-12, and a fixture confirms the producer states it
today: `...a` roots at `parameter`, `...b` at `parameter-or-own-result` with one
local-literal-result premise.

**The rows never closed.** `@corvu/utils` and `@corvu-next/utils`' `combineStyle`
carry 80 recipe-less `reads` rows in the 2026-09-14 pin — the largest
homogeneous block in that frontier — and `@corvu/utils` is published verbatim by
several packages, so the same eighty are the same code.

The reason is one the census stated about itself and nobody re-read:

> No deferral. The `creates` census holds a coercion or a local literal result
> back until its call walk has demanded the callees' transcripts; **this census
> has no call walk**, so a form it cannot decide here it cannot decide at all.

Confirming `parameter-or-own-result` means reading `stringStyleToObject`'s own
transcript: the premise names a call, and binding it requires that callee's
transcript to place the allocation and every return inside the callee. The
`creates` census gets those transcripts because `acquire_census_local_transcripts`
runs it during acquisition and batches whatever it requests. `reads` was not in
that loop at all — the code said so plainly, *"`reads` reads the root
transcript's forms only and asks for nothing"* — so the transcript was never
acquired, the premise could never bind, and `census_form_disposition` answered
`None` for a derivation that was fully implemented one domain over.

An implemented premise, stated by the producer, unreachable from the domain
whose rows it was written for.

## Decision

The `reads` census asks for the callees its own premises name, and nothing else.

1. **Acquisition includes `reads`.** `acquire_census_local_transcripts` runs the
   `reads` census alongside `creates` and `callbacks` and batches the local
   declarations it requests, over the same bounded rounds.
2. **The census demands before it decides.**
   `census_reads_demands_own_result_callees` walks the reachable forms, and for
   every `subject_local_literal_results` premise binds the named call to a local
   declaration and requests that transcript when it is missing. Any request at
   all returns `NeedsTranscripts` — asking is finished before any form is
   decided, so "this form's callee is missing" and "this form refuses" can never
   arrive as the same answer.
3. **The form is decided by ADR 0093's own check.** A form with own-result
   premises is dispositioned by `census_parameter_or_own_result_is_bound`, which
   is unchanged and shared with `creates`. Nothing is relaxed for this domain:
   the accessor kinds only, every premise bound, the same witness lines.
4. **A missing transcript at verification refuses**, naming the declarations,
   exactly as the `creates` branch does.

## What this is not

**It is not a call walk.** `creates` dispositions every call it reaches and
recurses through them; this asks only for callees a premise the producer already
stated names. A form carries that premise or it does not, so nothing here
decides which calls matter. The census still reports `calls: 0, depth: 0`,
truthfully: it dispositions no call and recurses into no declaration.

**It is not a new fact.** The producer states `parameter-or-own-result`
unchanged, so the handshake protocol does not move and no producer rebuild is
required by this ADR. That is the whole point — the fact was already on the
wire.

## Consequences

Eighty rows on `combineStyle` across two packages, plus whatever else in the
corpus states the derivation on a `reads` closure.

The shape this does *not* reach is the neighbouring one: a subject that is a
local binding initialized from a call, or a join expression rather than a
reference (`getComputedStyle`'s `getWindow(element).getComputedStyle`,
`toObserver`'s ternary). Those refuse as `call-result` and `not-a-reference` in
the producer and are separate premises; this ADR changes nothing about them.

A `reads` census now costs acquisition rounds it did not before, bounded by
`MAX_COMPOSITION_DEPTH` exactly as the other two are. Every export whose `reads`
forms state no own-result premise requests nothing and is unaffected.
