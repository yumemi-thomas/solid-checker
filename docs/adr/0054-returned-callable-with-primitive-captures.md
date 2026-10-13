# ADR 0054: One returned callable with primitive captures

Status: implemented; full `make verify` passed in 216.74 seconds. Corpus
first-refusal movement confirmed, with zero complete-candidate gain yet.

## Decision

The creates census may follow a stable local binding initialized by a call
to a stable, same-file factory when the authenticated source facts establish
all of the following:

- The initializer is an exact direct call whose identifier reference resolves
  to the factory binding, and the factory binding directly holds a callable.
- The factory is synchronous and non-generating, with an expression body
  that returns exactly one arrow/function literal. No body search or lexical
  descendant approximation selects the returned callable.
- Every factory parameter is a plain binding, has no default, receives one
  exact argument, and has no rest/destructuring ambiguity. Each argument has
  an affirmative primitive or nullish runtime-value fact.
- No assignment or iteration target writes a captured factory parameter.
  Both factory and result bindings pass the ordinary complete write and
  redeclaration checks.

The returned body's ordinary census still owns every operation it can run.
Other captured variables receive no new caller-input or callback exemption.
This is a first return-identity/capture derivation, not permission to trace
arbitrary factory results or treat the factory's return type as execution
evidence. Factories with block bodies, multiple returned targets, object or
callable arguments, mutable captures, and external factories remain open.

## Producer identity gap

The first native prototype reached the correct returned arrow but refused
with `no control-flow census at depth 1`: the producer required a variable
name for a local implementation demand. An anonymous arrow already has its
own compiler signature symbol. Protocol 37 admits that symbol only when its
declaration set consists of the exact demanded node. The resolved declaration
location independently binds the answer; a shifted or containing demand
cannot choose a nearby arrow or borrow the enclosing factory's identity.

No wire field is added. The handshake number and schema digest move with the
expanded meaning of a local-declaration answer. The producer's focused test
requires the exact anonymous declaration, nonempty symbol and control-flow
census, then shifts the span and requires an explicit open answer.

## Receipt and verifier

The native verifier resolves the initializer and factory through normalized
symbol-reference facts over authenticated runtime bytes. It records a
`census-factory-return:` site naming the result binding, initializer call,
factory node, exact returned callable, and every parameter-to-argument span
pair with its stated runtime-value kind. The normal local-declaration witness
then binds the returned body's Type Facts transcript digest and complete
execution census. A found returned arrow alone never constructs acceptance.

The factory expression only constructs the returned callable; default/rest
parameter execution is excluded by the premises above. Captured parameters
retain their primitive identity because writes are excluded. Reads or calls
through other captures remain subject to the existing census and can still
refuse.

## Regression and target

The packed native fixture certifies a `startsWith` predicate returned by a
factory called with a string literal. It keeps open a written factory, a
written result binding, a written capture and an object capture. Each runs
the live producer, creates census, synthesized veto and receipt finalizer.

The targeted corpus leaf is `checkStringStartsWith` behind
`isCSSVariableName` in motion-dom. Broader easing factories also capture
callables or nested factory results and are deliberately not credited to this
slice before measurement.

Full measurement: returned-callable first refusals 52 → 46, template
coercions 7 → 13, withheld candidates unchanged at 462. All six affected
`buildHTMLStyles` cases now reach `buildTransform`, whose optional
`TransformTemplate | undefined` premise fails to resolve in its twin. The
[measurement and exact next blocker](../package-contract-v2/phase21/2026-09-07-factory-census-measurement.md)
retain every occurrence and distinguish a cleared first refusal from an
accepted closure.
