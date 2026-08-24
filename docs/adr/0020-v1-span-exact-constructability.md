---
status: accepted
---

# V1 adds span-exact constructability

## Decision

The sole active lifecycle schema, V1, adds a demand-shaped `constructability`
fact: whether the type at one exact demanded span has *construct* signatures.
Wire table schema v14 carries it; earlier table schemas remain frozen and
decodable. The handshake protocol stays 2 — this adds a fact to an existing
operation rather than an operation a peer must know about — so only the schema
digest and the build id move, exactly as every other vocabulary change before
the `modules` one did.

The named follow-up below has since landed, and it moved the wire table schema
to v15 and the digest again: `callability` gained one value, `untypedCallable`.
That is recorded here rather than in an ADR of its own because it is the closure
of this ADR's own residue and changes no row layout — see "Follow-up delivered"
at the end.

`constructability` is `callability` asked of `SignatureKindConstruct`. It reads
the same `GetTypeAtLocation` type, distributes the same real union constituents
with `Type.Distributed`, fails closed on the same type flags (`any`, `unknown`,
`never`, missing), and reports `constructable`,
`nonConstructable`, `mixed`, or `unknown`. Aliases, imports, and re-export
specifiers are transparent because the type is. No `TypeToString` result
participates. A compiler error type is caught by the `any` check above, not by
a live error flag. The adapter's mask also tests `TypeFlagsIncludesError`
defensively, but that bit cannot fire there: TypeScript-Go's `errorType` is
constructed with only `TypeFlagsAny` set
(`c.newIntrinsicType(TypeFlagsAny, "error")`), and `TypeFlagsIncludesError` is
a transient accumulator `getUnionTypeWorker` sets on a local `includes`
bitmask while *building* a union — it is read there to choose `errorType` as
the union's result, then discarded; it is never written to any retained
`Type.flags`. An earlier revision of this ADR described the fail-closed path
as keying on that flag; it does not, and the `any` bit is what actually does
the work.

Compact demand bit 16 selects it. Entity flag bit 13 carries a one-word tag in
its own code space rather than borrowing callability's, so neither fact can be
decoded as the other if either vocabulary grows. Full, delta, retained-reuse,
equality, and demand-hash paths all carry the new value.

## Why the fact is separable from callability

Construct signatures are precisely what `callability` does not count, which
leaves a class's *value* type unanswerable. `typeof C` for `class C {}` is
`nonCallable`, and `typeof C === "function"` holds at runtime. A consumer that
must decide whether an export is a runtime function therefore had two options
before this fact: read `callability` and be wrong about every class, or search
the syntax for a `class` keyword. The syntactic search is what a consumer
actually built, and it is defeated by a bundler: a class published as
`const C = /* @__PURE__ */ (() => { class C {…}; C.prototype[X] = true; return C; })()`
has an initializer that is a *call*, and a class reached as a tuple element type
declared in another package has no class expression in the analyzed artifact at
all. Neither shape has anything to find. The type has the answer, and it is one
`GetSignaturesOfType` call away from the one already being made.

Keeping it a *separate* fact rather than widening `callability`'s vocabulary
preserves every frozen wire code and every existing consumer's reading of
`nonCallable`, and lets a consumer that only asks "can I call this" avoid the
second signature walk.

## What it does not answer

The two facts aggregate independently over the constituents, so a `mixed`
verdict on either does not compose with the other into a per-constituent proof.
`(() => void) | number | (new () => X)` answers `mixed` twice over and still
holds a constituent that is neither callable nor constructable. A consumer
needing "every constituent is a function" cannot get it from these two facts and
must fail closed on `mixed`.

`unknown` remains the absence of an answer rather than a negative one, so this
fact does not rescue a type the checker cannot close. A downleveled TypeScript
enum (`var E; (function (E) {…})(E || {});`) and a value computed from an untyped
global are `any` in the analyzed artifact: `callability` reports `unknown` there
and so does this fact. Their `.d.ts` answers definitively and neither fact reads
it. That gap belongs to a primitive- or object-domain fact demanded at
export-specifier spans, not to this one. solid-js@1.9.14's `./web` entry is the
single most consequential instance the precision backlog names: all 76 of its
exports resolve through `Object.create(null)` to `any`, so `callability` and
`constructability` are both `unknown` for every one of them and the pair
refuses the entire module rather than guessing.

Abstract construct signatures are deliberately not filtered out — an abstract
class is still a function object at runtime — so the fact does not answer
instantiability. And a class *declaration name* is not the same span as an
export specifier: the compiler's type at the name is the class's instance type,
which is honestly `nonConstructable`. Both are pinned by tests.

`nonCallable` + `nonConstructable` together proved only that the *declared
type* carries no call or construct signature of its own — not that the runtime
value cannot be a function. lib.es5.d.ts's `Function` interface declares
`apply`/`call`/`bind` and no signature, so `export declare const x: Function`
answered this pair `nonCallable` + `nonConstructable` even though every function
is assignable to `Function`. **That residue is closed** — see "Follow-up
delivered" below, which is also where the family turned out to be narrower than
this paragraph originally claimed.

## Performance and representation

Neither retained entity row grew. The Rust `EntityFact` stays at 144 bytes:
`Option` over a four-member fieldless enum landed in existing padding, so the
144-byte gate holds without being raised. The Go `EntityFact` stays at 200 bytes,
which took a deliberate divergence from `Callability`: as a string the field cost
every row 16 bytes to carry an absence (200 → 216, and +4.40% cold allocated
bytes at scale), so it is a compact integer stored in the padding beside
`PrimitiveValueDomain` instead — the same call that fact made, for the same
reason. Its zero value is the fact's absence.

`EntityDemand` did grow, 56 → 64 bytes: the seventeenth flag crossed an alignment
boundary, and packing that struct's flags is a separate change. That is the whole
residual cost.

The extra `GetSignaturesOfType` walk runs only when the fact is demanded, and it
reuses the type the demand already resolved — no project-wide type walk was
added, and a demand for `constructability` alone resolves exactly one type.

Measured on an Apple M4 Pro against `e2f7ac5`, same command, corpus, warm edit,
and five-run median (`make benchmark-memory`): cold full-table analyze
3,562,405 → 3,552,378 ns (-0.28%), warm leaf edit 649,415 → 649,172 ns (-0.04%),
its analyze portion 259,526 → 258,862 ns (-0.26%). Response bytes are unchanged
(cold 129,428 → 129,426; warm 3,405 → 3,405) and median allocation counts are
unchanged (3,465/2,773). Cold allocated bytes rose 1,880,274 → 1,921,897
(+2.21%), which is the demand struct's eight bytes across the corpus's demands
and nothing else.

## Consequences

Consumers can prove that an exported binding is a runtime function without a
syntactic class search, and — with `nonCallable` and `nonConstructable`
together — can prove that one is not, because the family that used to satisfy
that conjunction while being callable now answers `untypedCallable` instead.
They must remain fail-closed on `unknown`, on `mixed`, and on absence; must
treat `untypedCallable` as a function whose signature may not be read; and must
demand the fact at the export-specifier span rather than at a declaration name.
Bundler lowering, runtime prototype patching, and instantiability remain outside
this structural fact.

## Follow-up delivered: the signature-less `Function`-supertype family

The follow-up this ADR named — carry `isFunctionObjectType`'s `bind`-member
subtype-of-`Function` fallback into the facts — landed, but not as named. The
compiler was measured first, and it contradicted the sketch in three ways.

**The family is narrower than the prose claimed.** Asked of the compiler's own
`isTypeSubtypeOf` against the global `Function` type at the pinned revision:

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

**The answer is not `unknown`, and not `callable`.**
`declare const f: Function; f()` compiles: the compiler resolves it through TS
1.0 §4.12 (`checker.isUntypedFunctionCall` — no signatures of either kind, not a
union, assignable to the global `Function` type) and hands it `anySignature`. So
`nonCallable` claimed the call was illegal where the compiler allows it, and
`unknown` would have claimed no domain was closed where one was. `callable` was
wrong in the other direction: consumers read `callable` as "there is a signature
here" and pair it with `resolvedCall` and parameter facts. The fact therefore
gained a fifth value, `untypedCallable` — callable, with nothing about the call
readable. `callable`, `nonCallable`, `mixed` and `unknown` keep their exact
previous meanings and wire codes.

**Only callability moved.** `new f()` on the same value does *not* compile:
`resolveNewExpression` has no untyped fallback, unlike call, tagged-template and
decorator resolution. Constructability's `nonConstructable` for this family is
therefore the compiler's own answer, and giving it a matching value would have
invented a claim the type system contradicts. The pair's two halves disagree
about exactly one type family, and the disagreement is upstream's. Both halves
are pinned against the compiler's own diagnostics
(`TestTheFunctionSupertypeFamilyIsCallableButNotConstructable`).

**The derivation is the call rule, not the narrowing predicate.** The two agree
everywhere except an intersection: `Function & { brand: "route" }` is a subtype
of `Function` and its call is permitted, yet `isFunctionObjectType` answers false
there, because its `bind` quick-out reads
`resolveStructuredTypeMembers().members` and the compiler leaves that map empty
for every intersection by construction ("The members and properties collections
are empty for intersection types"). A predicate that answers false for a callable
type would have kept `nonCallable` — a false negative — for a plausible branded
handler, so callability asks the rule that decides the call. Four `go:linkname`d
identifiers carry this: `isUntypedFunctionCall` and `getReducedApparentType` in
the derivation, `isTypeSubtypeOf` and `isFunctionObjectType` in the tests that
pin the boundary as a relation rather than as a list of type names.

Aggregation gained one rung: `untypedCallable` sits below `callable` and above
`mixed`. Constituents that are all callable in either sense answer the weaker of
the two, so `Function | (() => void)` is `untypedCallable`; a non-callable
constituent beside a callable one is still `mixed`, so `Function | number` and
`Function | undefined` are `mixed` where both were `nonCallable` before.

### Wire and representation

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

### Residues this did not close

- **`runtimeValueDomain` still answers `mayBeCallable: false` for the family.**
  It is a separate fact with separate consumers and its own aggregation; the same
  fallback would fit it, and it is deliberately outside this change. A consumer
  needing "is this a function" should ask `callability`.
- **A type parameter whose constraint is `any`.** The compiler's untyped-call
  rule has a disjunct that admits it, and the derivation refuses to follow it
  there: an `any`-derived positive is not an answer this fact may produce, so such
  a constituent keeps what it answered before — `nonCallable`. That is the same
  class of false negative the family had, and closing it belongs with the `any`
  residue this ADR already names rather than here.
- **No corpus measurement of the family's size.** The before/after target named
  in the consumer's precision backlog — a search for exports typed against the
  family — is still unmeasured; `benchmarks/ecosystem/` was not run.
