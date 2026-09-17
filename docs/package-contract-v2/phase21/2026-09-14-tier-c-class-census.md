# Tier C census: a class export never reaches its own constructor (2026-09-14)

Measured before authoring anything, against the 1,884-row `reads` frontier.
The 2026-09-13 depth plan filed classes as needing "the census of a
constructor body". That census already exists. What is missing is one step
earlier, and it is much smaller than the plan assumed.

## The rows

`reads` only. No class export carries a withheld `creates`, `callbacks` or
`returns` row on this pin.

| package | export | rows | artifact cases |
| --- | --- | ---: | ---: |
| `@tanstack/store@0.11.1` | `Store` | 40 | 1 |
| `@tanstack/store@0.11.1` | `ReadonlyStore` | 40 | 1 |
| `motion-utils@12.39.0` | `SubscriptionManager` | 26 | — |
| `@tanstack/devtools-event-client@0.5.0` | `EventClient` | 11 | — |
| **total** | | **117** | |

The plan sized this at 160. The 80 `@tanstack/store` rows are a **single**
artifact case (`0bdfa1cb…`) reached from five roots — `@tanstack/solid-pacer`
(62), `@tanstack/solid-table` (8), `@tanstack/solid-form` (4),
`@tanstack/solid-hotkeys` (4), `@tanstack/solid-store` (2) — so one premise
closes all eighty at once.

## Measured verdict: zero gainable by authoring

Two-pass scaffold on `@tanstack/solid-pacer@0.22.0` (graph lane,
`--recover-entrypoints`, `--specifier @tanstack/store`), solid 1.9.14. Pass 1
emitted 12 modules; pass 2 reported **every one unserviceable**:

~~~
census refused: domain-exhaustiveness (artifact-case:0bdfa1cb…:Store):
  runtime implementation transcript is incomplete or open
  (reasons=["callSignatureNotUnique"])
~~~

`ReadonlyStore` and `EventClient` refuse identically. This confirms the plan's
classification — no recipe can serve a class export — and it names the
refusal, which the plan did not.

## Why `callSignatureNotUnique` on a class

`export_value_transcripts.go` selects the invocation like this:

~~~go
valueType := p.checker.GetTypeAtLocation(node)
signatures := p.checker.GetSignaturesOfType(valueType, checker.SignatureKindCall)
…
if len(signatures) != 1 {
    transcript.OpenReasons = append(transcript.OpenReasons, "callSignatureNotUnique")
    return transcript
}
~~~

A class is not callable. `SignatureKindCall` yields **zero** signatures, and
`!= 1` reports the same reason for "none" as for "several", so the transcript
returns before any implementation is looked for.

The construction census is on the other side of that early return.
`localDeclarationImplementationTranscriptLocked` already handles a class at the
demanded location — `exactClassDeclarationAt` matches a `ClassExpression` as
well as a `ClassDeclaration`, and `classConstructorAt` (protocol 55, ADR 0047's
line) either hands back the constructor or refuses on a heritage clause, a
field initializer, a static block, a computed member name, a decorator, a
parameter property, or an implicit constructor. None of that is reachable for
a class *export*; it runs only when some other census resolves a callee to a
class.

That machinery is also already correct for the shape the corpus has.
`@tanstack/store` ships `var Store = class { … }` — the bundler spelling —
which `classHeritageAndMembers` handles explicitly and by name. Applying
`classConstructorAt` by hand to both classes: no heritage clause, no
initialized `PropertyDeclaration` (the fields are assigned in the constructor
body), no static block, no computed name, no decorator, no parameter property,
and a constructor with a body. **Both would be admitted.**

## So the premise is signature selection, not a new census

Ask for `SignatureKindConstruct` when the target is a class, and when exactly
one construct signature is selected, let the existing class branch census the
constructor. That is a producer change with a protocol bump and a certifier
arm, but it is not the constructor-body census the plan budgeted for.

Two things must be got right, and one of them has already bitten:

- **The type at a class's own name is the instance type**, which has neither a
  call nor a construct signature; the exported value is the *constructor*. The
  ADR 0099 comment in `export_value_transcripts.go` records this from the
  `Box` fixture, where a synthesized `typeof` veto contradicted the stated
  fact. A construct-signature premise has to read the symbol's type, not the
  name's.
- **Overloaded constructors** must keep refusing, exactly as overloaded call
  signatures do.

What a construction actually runs is *not* re-litigated here; ADR 0047 and
`classConstructorAt` already drew that line, and this premise inherits it
whole.

## The shared refusal with ADR 0103, and what it now predicts

`Store` refuses with the **same reason code** ADR 0103 lifts for `Object.keys`:
`domain-exhaustiveness … callSignatureNotUnique`. That premise is now measured
closing **148** corpus rows, so the pattern "state a fact the signature check
cannot, then close on it in a reviewed certifier arm" is proven to carry a row
all the way to a certified contract.

One difference matters for sizing. ADR 0103's veto is an identity witness that
needs no signature at all, so synthesis serves it. A class premise produces an
ordinary `reads` closure, and synthesis is gated on
`evidence.call_signatures(&record.export)` — which a class does not have, for
exactly the reason this premise exists. So the class arm has to say what its
veto observes, and `reviewed_observation` registers nothing for `reads`
(`2026-09-10-reads-veto-observation-design.md` § 6: an unenumerated read is a
read of a source the export *owns*, which a synthesized veto cannot
instrument).

That leaves two honest options, and they should be decided before any producer
code is written:

1. **A hand recipe per class export.** The pass-2 run says no recipe can serve
   these candidates *today*, but that verdict is the census refusing — once the
   construct-signature premise lifts it, the candidates become decidable and a
   hand recipe is exactly what closes them, as it did for 358 Tier A rows.
   Four exports, one artifact case for eighty of the rows.
2. **A construction-identity observation**, the way ADR 0103 found one for
   aliases. What a construction could be observed to *not* do is not obvious,
   and inventing one is the failure mode § 6 warns about.

Option 1 is the smaller, better-evidenced path.

## Not measured here

`motion-utils@12.39.0` (`SubscriptionManager`, 26 rows) stays unmeasured: its
only corpus root is `motion-solidjs@0.7.0-beta.4`, whose graph finalization
hits the recorded "runtime implementation does not match the snapshot-replayed
export binding" blocker. `EventClient` (11 rows) was measured incidentally —
it appears in the `@tanstack/solid-pacer` graph and refuses identically.
