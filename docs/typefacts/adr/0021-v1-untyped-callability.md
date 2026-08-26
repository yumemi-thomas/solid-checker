---
status: accepted
---

# V1 adds untyped callability for the signature-less `Function` supertypes

## Decision

`callability` gains a fifth value, `untypedCallable`: a type the compiler
permits calling although it exposes no call signature to read. Wire table schema
v15 carries it as callability tag 4; earlier table schemas remain frozen and
decodable, and emission at those schemas degrades the value to `unknown`. The
handshake protocol stays 2 — this widens a closed tag space inside an existing
operation rather than adding an operation a peer must know about — so only the
schema digest and the build id move.

This closes the residue ADR 0020 named: `nonCallable` + `nonConstructable`
proved only that the *declared type* carries no call or construct signature, not
that the runtime value cannot be a function, because lib.es5.d.ts's `Function`
interface declares `apply`/`call`/`bind` and no signature of its own. ADR 0020
sketched the fix as "carry `isFunctionObjectType`'s `bind`-member
subtype-of-`Function` fallback into the facts". It landed, but not as sketched:
the compiler was measured first, and it contradicted the sketch in three ways.

## The family is narrower than ADR 0020's prose claimed

Asked of the compiler's own `isTypeSubtypeOf` against the global `Function` type
at the pinned revision:

| demanded type | subtype of `Function` | `isFunctionObjectType` | untyped call permitted |
| --- | --- | --- | --- |
| `Function` | yes | yes | yes |
| `CallableFunction` | yes | yes | yes |
| `NewableFunction` | yes | yes | yes |
| `type Handler = Function` | yes | yes | yes |
| `interface M extends Function {…}` | yes | yes | yes |
| `Function & { brand: "route" }` | yes | **no** | yes |
| `object` | **no** | n/a (not a structured type) | no |
| `{}` | **no** | no | no |
| `Record<string, unknown>` | **no** | no | no |
| `interface OnlyBind { bind(…): void }` | **no** | no | no |
| `number` | no | n/a | no |

So `object`, `{}` and `Record<string, unknown>` were never in the family: a
function value is assignable *to* `object`, but `object` is not assignable to
`Function` and the compiler refuses to call it. `nonCallable` is the honest
answer for all three, and a consumer publishing `value` for
`export declare const x: object` is repeating the declared type's own claim — no
consumer of that export can call it either. That is now the documented boundary.
A `bind` member alone is not the rule either: it is only the compiler's cheap
pre-filter before the relation that is.

## Why the answer is not `unknown`, and not `callable`

`declare const f: Function; f()` compiles: the compiler resolves it through TS
1.0 §4.12 (`checker.isUntypedFunctionCall` — no signatures of either kind, not a
union, assignable to the global `Function` type) and hands it `anySignature`. So
`nonCallable` claimed the call was illegal where the compiler allows it, and
`unknown` would have claimed no domain was closed where one was. `callable` was
wrong in the other direction: consumers read `callable` as "there is a signature
here" and pair it with `resolvedCall` and parameter facts. Hence a fifth value —
callable, with nothing about the call readable. `callable`, `nonCallable`,
`mixed` and `unknown` keep their exact previous meanings and wire codes.

## Why only callability moved

`new f()` on the same value does *not* compile: `resolveNewExpression` has no
untyped fallback, unlike call, tagged-template and decorator resolution.
Constructability's `nonConstructable` for this family is therefore the compiler's
own answer, and giving it a matching value would have invented a claim the type
system contradicts. The pair's two halves disagree about exactly one type family,
and the disagreement is upstream's. Both halves are pinned against the compiler's
own diagnostics (`TestTheFunctionSupertypeFamilyIsCallableButNotConstructable`).

## Why the derivation is the call rule, not the narrowing predicate

The two agree everywhere except an intersection: `Function & { brand: "route" }`
is a subtype of `Function` and its call is permitted, yet `isFunctionObjectType`
answers false there, because its `bind` quick-out reads
`resolveStructuredTypeMembers().members` and the compiler leaves that map empty
for every intersection by construction ("The members and properties collections
are empty for intersection types"). A predicate that answers false for a callable
type would have kept `nonCallable` — a false negative — for a plausible branded
handler, so callability asks the rule that decides the call. Four `go:linkname`d
identifiers carry this: `isUntypedFunctionCall` and `getReducedApparentType` in
the derivation, `isTypeSubtypeOf` and `isFunctionObjectType` in the tests that
pin the boundary as a relation rather than as a list of type names.

## Aggregation

Aggregation gained one rung: `untypedCallable` sits below `callable` and above
`mixed`. Constituents that are all callable in either sense answer the weaker of
the two, so `Function | (() => void)` is `untypedCallable`; a non-callable
constituent beside a callable one is still `mixed`, so `Function | number` and
`Function | undefined` are `mixed` where both were `nonCallable` before.

At a union this promise is per constituent — "every constituent is callable in
some sense" — and it is deliberately weaker than either "the call as written
compiles" or "no constituent's signature is readable". Both directions are
measured, not asserted:

- `Function | (() => void)` still carries one readable, arity-enforced call
  signature: `declare const a: Function | (() => void); a(1);` is TS2554
  ("Expected 0 arguments, but got 1"), so a signature *is* readable here even
  though the fact reports `untypedCallable`.
- `Function | Merged`, where `Merged` is itself in the untyped-call family
  (for example a `declare class C {}` merged with `interface C extends
  Function {}`), has tsc refuse the call outright: `declare const b: Function
  | Merged; b();` is TS2349 ("This expression is not callable"), because the
  untyped-call rule that grants each constituent its own fallback signature
  explicitly excludes unions — so the whole union never gets that fallback,
  even though every constituent individually qualifies for it.

Either way the answer stays conservative: a consumer that reads
`untypedCallable` as "callable, signature unread" only under-checks what it
could have proven in the first case, and does not itself assert that the call
as written type-checks in the second. Both shapes answered `nonCallable` or
`mixed` before this rung existed, which was a worse answer in the opposite
direction — a real false negative on a callable value, not an imprecise
positive on an uncallable union.

A design that closed this gap would ask the union type itself, rather than
its constituents, whether it exposes call signatures, and answer `callable`
there when it does (matching `Function | (() => void)`'s real signature) and
`nonCallable`/some new value when the untyped-call rule's union exclusion
leaves it with none (matching `Function | Merged`). That is a real precision
gain and a deliberate scope cut this ADR does not take: it would change what
"union of untyped-callable constituents" means rather than just documenting
it, and the corpus impact of the two shapes above is unmeasured.

## Wire and representation

No row layout changed and nothing grew: the Go `Callability` is already a string
and the Rust one already an enum, and `TupleShape` packs `elementZero` in three
bits of which only five values were used. What changed is a closed tag space —
callability tag 4 exists from Wire table schema **v15**. The version, not a flag
bit, carries that: a v14 decoder cannot express the value, so it refuses tag 4,
and emission at v14 or earlier degrades `untypedCallable` to `unknown` rather
than to `nonCallable`, keeping every frozen schema exactly decodable and never
turning an absent answer into a negative one. The handshake protocol stays 2; the
schema digest and the build id move, as they do for every vocabulary change.

The producer and client ship in build-ID lockstep, so a v15 payload cannot reach
a v14 client in practice. The version discipline is what makes that a guarantee
rather than an accident.

The derivation's cost is one extra apparent-type lookup and one assignability
query per constituent that has no signature of either kind — never for a
constituent that does. Measured on an Apple M4 Pro against `3296ec8c` in the
same session, same corpus and warm edit, five-run median
(`make benchmark-memory`): cold full-table analyze 3,741,493 → 3,725,597 ns
(-0.42%), warm leaf edit 679,116 → 670,265 ns (-1.30%), its analyze portion
269,091 → 274,594 ns (+2.04%) — all three inside this gate's run-to-run spread.
Response bytes are unchanged (cold 129,428 → 129,428; warm 3,405 → 3,405) and so
are cold allocations (3,471 → 3,470 count, 1,922,746 → 1,922,870 bytes). The one
real cost is on the warm leaf edit, whose relation caches start cold each
generation: 2,773 → 2,811 allocations (+1.37%) and 801,550 → 815,473 allocated
bytes (+1.74%).

## Consequences

`nonCallable` + `nonConstructable` is now proof that the declared type carries no
signature of either kind *and* that the compiler refuses to call it, which is as
close to "the value is not a function" as a declared type can come. Consumers
must handle `untypedCallable` as a positive function answer whose signature,
arity, and parameter types may not be read, and must not read
`constructability` alone as "not a function" for such a row. They remain
fail-closed on `unknown`, on `mixed`, and on absence.

## Residues this did not close

- **`runtimeValueDomain` still answers `mayBeCallable: false` for the family.**
  It is a separate fact with separate consumers and its own aggregation; the same
  fallback would fit it, and it is deliberately outside this change. A consumer
  needing "is this a function" should ask `callability`.
- **A type parameter whose constraint is `any`.** The compiler's untyped-call
  rule has a disjunct that admits it, and the derivation refuses to follow it as
  a defensive guard rather than a closed gap: measured, `function f<T extends
  any>(x: T) { x(); }` reduces `T`'s apparent type to `unknown`, not `any` — the
  disjunct never actually fires — and the compiler itself refuses the call
  (TS2349, "Type 'unknown' has no call signatures"). So the constituent falls
  through to `nonCallable` regardless, which *agrees* with tsc: there is no false
  negative here, unlike the family this ADR does close.
- **No corpus measurement of the family's size.** The before/after target named
  in the consumer's precision backlog — a search for exports typed against the
  family — is still unmeasured; `benchmarks/ecosystem/` was not run.
- **A union that asks its constituents rather than itself.** Answering
  `untypedCallable` per constituent is deliberately weaker than asking whether
  the union type has its own readable call signature; that stronger design —
  described under Aggregation above — is a real precision gain not taken here.
- **The frozen-schema degradation path exists for goldens and replay, not for a
  live mismatched peer**: the session handshake already refuses any producer
  whose protocol, schema hash, or build ID does not match exactly, so a v15
  payload never actually reaches a v14 client in practice — the v14 decode path
  this ADR keeps decodable is for byte-frozen fixtures such as the phase1 CBOR
  golden, not for cross-version interoperability.
- **The phase1 CBOR golden (`benchmarks/phase1/typefacts-v3-response-golden.cbor`)
  is a Wire table v14 freeze**; there is no equivalent cross-language golden for
  v15 or `untypedCallable` yet.
- **The interface-merging positive is faithful to tsc but not to the runtime**:
  a `declare class C {}` merged with `interface C extends Function {}` answers
  `untypedCallable` because tsc permits calling it, but the merge only asserts
  the shape at the type level — the fact inherits whatever lie the author's
  declaration tells about `C`'s actual runtime behavior.
