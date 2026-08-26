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

The follow-up this ADR named has since landed as ADR 0021, which moved the wire
table schema to v15 and the digest again: `callability` gained one value,
`untypedCallable`. Read that ADR for the family's measured boundary, the
derivation, and the wire consequences; this one describes constructability at
v14 and the residue it left.

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
is assignable to `Function`. **That residue is closed by ADR 0021**, which is
also where the family turned out to be narrower than this paragraph originally
claimed.

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
that conjunction while being callable now answers `untypedCallable` instead (ADR
0021). They must remain fail-closed on `unknown`, on `mixed`, and on absence;
must treat `untypedCallable` as a function whose signature may not be read; and
must demand the fact at the export-specifier span rather than at a declaration
name. Bundler lowering, runtime prototype patching, and instantiability remain
outside this structural fact.
