# ADR 0113: A `returns` closure over a primitive completion

- Status: accepted and implemented (2026-09-23); written with the implementation
- Date: 2026-09-23
- Owners: the generator's `returns` proposal (`returns_walk.rs`,
  `main.rs`'s walk index, `inferred_contract.rs`), the policy-2 `returns`
  census and the positive fact beside it (`type_facts.rs`), the synthesized
  vetoes (`synthesized_vetoes.rs`), and the consumer's `returns` projection
  (`contracts.rs`)
- Relation: the fourth decidable shape of ADR 0035's `returns` census, beside
  its empty closure, ADR 0075's whole-parameter identity and ADR 0109's merged
  props root. It reads ADR 0045's `primitiveCompletion` and the control-flow
  census's per-site values, and its veto follows ADR 0096's synthesized
  pattern. **No producer change**: no handshake protocol, no schema field.

## Context

`phase22/2026-09-23-what-holds-an-import-open.md` classified every open domain
a consumer's import finds, for the 994 in-surface sites the census's consumer
view does not clear. On **887** of them `returns` is open with no proposal and
no decline record: the generator proposed a `returns` closure in exactly three
shapes (empty, a whole parameter, a props merge), and anything else a function
returns — `() => true` included — left the domain unknown. No ledger saw it,
because every ledger was built from withheld closures and a claim nobody
proposed is never withheld.

Nine exports have `returns` as their only open consumer domain. Six of them
return a primitive on every completion — `@solid-primitives/utils`' `noop`,
`trueFn`, `falseFn`, `isObject` and `clamp`, and `@kobalte/utils`' `clamp` —
161 sites that would be the first callables a consumer imports with nothing
open. The census's `clean` bucket was 0.

## Decision

**`returns` closes over exactly one `return` whose output is `plain` when the
producer's types say every completion the body can reach hands back a
primitive, and every declared overload agrees.** `plain` is the semantic
model's value that "carries no reactive capability" (§ Recursive value shapes).
A primitive is the narrow case of it this ADR can prove, and nothing wider is
claimed: a plain *object* can carry an accessor, so it is never admitted here.

### The generator proposes; it cannot decide

`returns_walk::value_completion` is the valueless-completion walk's other
positive answer. It holds for a function that walk declined **on a value** — a
`return` carrying an expression in the function's own body, or an expression
body — when no such completion is a literal that is an object on every run: a
function or arrow literal, or an array or object literal the facts record an
element or property of. Those would only spend a census refusal, and
`chainCallbacks`, `debounce` and `{ view: 1 }` are exactly that. `async`
functions and generators never answer: they hand the caller a promise or an
iterator whatever the body returns. A body the walk cleared is ADR 0035's
`returns: []`, never a plain return.

`ContractExport::returns_value_completion` carries the answer to the emit
boundary. `inferred_contract.rs` proposes `returns` closed over one
`kind: return` operation — `output: plain`, at the call, same stack, untracked,
`count: {scope: call, min: 0, max: many}` — for a function summary whose
reactive analysis described no return (`Known(None)`), in a scope that
publishes bootstrapped domains, not inherited and not `async`, and the
proposable filter admits the closure. The walk sees syntax: it cannot tell
`clamp` from `passThrough`, and the census decides.

### What the census decides

The fourth arm of `census_returns_domain`, last because it is the least
specific: a whole parameter or a props merge that happened to be a primitive
would also be plain. Its premises, each refusing by name:

1. a plain completion form and a present, classified control-flow census,
   exactly as ADR 0035 and ADR 0109 require (`require_plain_classified_completion`);
2. the producer's `primitiveCompletion`;
3. every value-carrying return the producer did not prove unreachable carries a
   value fact that states a primitive **alone**: closed (no open reason, every
   partition complete), not `unknown`, never an object, callable or
   constructible, and naming at least one primitive type — `never` names none
   and is refused, since it states there is no value at all;
4. at least one such return, so the one operation the closure enumerates can
   occur. A body with none closes `returns: []` instead.

A bare `return;` and falling off the end hand back `undefined`, a primitive,
and by the model's § returns are not `return` operations, so they need no
disposition.

### Why every return site, not `primitiveCompletion` alone

Measured on the fixture's `annotatedBox`:

```js
/** @returns {number} */
export function annotatedBox() { return {}; }
```

**The producer states `primitiveCompletion` for it.** The fact is the checker's
return type for the signature, and in a JavaScript file that is whatever a JSDoc
`@returns` says: the checker does not hold an annotation to the body. The return
expression's own type is `{}`, and premise 3 is what refuses it. The
control-flow census computes each site's value on the original program —
`exportImplementationTranscriptLocked` builds it before, and independently of,
`premisedFormCensusLocked` — where an unannotated parameter is the implicit
`any`, so an expression typed a primitive there is one whatever the caller
passes. `primitiveCompletion` stays a premise because it can only refuse more.

### A premised transcript is decided by the same evidence

ADR 0038's premise replaces the forms and `primitiveCompletion` — which on the
twin restates the declared `@returns` — and never the control-flow census.
`sign(value) { if (value === 0) return; return value > 0 ? 1 : -1; }` records a
coercion, is premised, and certifies on its sites. A first cut refused every
premised transcript; that withheld `sign` and bought nothing, because the
evidence the census reads is not the premise's.

### The operation stands on its own

The `return` is a positive claim with its own demand (a recursive value rooted
at the operation's output, callability `NonCallable`). A refused closure would
leave it a partial claim that still says what a return hands back, so it cannot
lean on the census: it reads the same evidence (`primitive_return_sites`, shared
so the two cannot drift) **and** every declared overload's result, which must
be a primitive alone. The declaration's non-callability — what the generic
signature arm accepts for a `plain` output — is not enough: an array is not
callable, and the array an export returns may be its caller's store.
`widened` (`return 1`, declared `number | object`) refuses there.

When the evidence refuses, the operation is withdrawn by name (`operation
census refused: …`), and the domain that listed it opens with it: the document
stops stating a plain return at all. That is the record a refusal leaves; the
closure itself is not separately withheld.

### The veto

`Observation::PrimitiveReturn` is synthesized for the closure (ADR 0036). It
calls every sample tuple the declared signatures admit, at most six per
overload, and emits `return-not-primitive` when a normal completion's `typeof`
is `"function"`, or `"object"` and the result is not `null`. `typeof` is an
operator, so a package that replaces a global during its import cannot bend
the check, and a `Proxy` still reports its kind. A throwing sample observes
nothing, and a run in which no sample completes normally is incomplete, never
satisfied by silence: `implementation-census-creates`' `instanceOfParameter`
and `instanceOfModuleValue` throw on every call, their right-hand sides being
a sampled non-constructor and `undefined`, and their closures are withheld for
it. A contradiction refuses the row (ADR 0036 § 2).
Finite observation proves nothing; the census does.

### The consumer reads it

`project_return` (`contracts.rs`) projects an accepted `returns` claim into the
consumer's `ContractClaim<Option<ContractReturn>>`, and `plain` has no return
kind there, so a closed claim over one plain return projected to "no shape" and
the domain was marked **open** again: the Rust consumer kept raising `SC9005`
for exactly the exports this ADR closes, while the census, which reads the
wire's `closed` list, counted them clean. A plain value carries no reactive
capability, which is what the consumer's own `Known(None)` states for a
function whose reactive analysis described no return, so a closed claim whose
every return is plain now projects to that. An open claim, or a closed one over
an output the projection cannot represent, still opens the domain.
`fixtures/reactive-ir/package-plain-return-consumer` pins both halves on an
authorized contract: `isReady` is clean, and the identical `isReadyOpen`, with
`returns` open, raises `SC9005`. Built without this projection, the fixture
reports `SC9005` for both.

## Where the trust sits

**The checker's type of each return expression, over `any` parameters.** A
primitive there comes from a literal, an operator TypeScript types primitive, a
comparison, `typeof`, `instanceof`, or a callee's return type: the default
library (`Math.min`), a `.d.ts` the package imports, or a local helper's own —
inferred, or a JSDoc `@returns` on *that* helper, or a JSDoc `@type` cast, all
of which the checker takes at its word. Those last are the trust ADR 0045
already extends to a callee's completion. This ADR closes the case where the
annotation sits on the export itself; for the others the veto is the only
backstop, and only on its samples. The declared `.d.ts` is read only to refuse.

**In a TypeScript source artifact the parameters are not `any`.** They carry
their written types, the site types rest on them, and the claim holds for a
caller inside the declared signature, which is the condition ADR 0038's premise
already places on a JavaScript closure. Such a case cannot certify today in any
event: the probe harness refuses to load TypeScript under `node_modules`, so its
veto does not complete and the closure is withheld.

## What still refuses

- An object, the caller's own value, anything the producer types `any`
  (`box`, `passThrough`), and a literal the walk rules out, which is never
  proposed.
- **`a + b` over untyped operands.** `+` always yields a string, a number or a
  bigint at run time, but TypeScript types it `any` when an operand is `any`,
  and the census reads the type (`add`). `-`, `*` and comparisons are typed
  primitive and certify (`declaredMemberCoercion`'s `axis.max - axis.min`).
- **Outputs that are, or carry, something else**: a new array holding the
  caller's value (`asArray`), the caller's value or its call's result
  (`accessWith`), a returned function (`createIdGenerator`). Each needs a
  described shape of its own, as ADR 0075 and ADR 0109 are; a later step.
- A primitive return beside another return shape: the closure enumerates one
  operation, exactly.
- `async` functions and generators, and a construction (ADR 0105): `new` hands
  the caller the instance whatever the constructor body returns, so a
  construct transcript refuses before any site is read. The same day this
  moved into the premise every `returns` arm shares, and into the
  whole-parameter proof, which had not checked it.

## Consequences

- Contract corpus: 43 of 97 fixtures move, each only by the new proposal.
  Possible operations 174 → 432, proof candidates 1,259 → 1,512, local open
  claims 5,023 → 6,058 (every new operation carries its five owner-axis open
  claims; the `returns` claims it now proposes closed leave). No refusal sidecar
  moves, and no existing coverage finding moves either: nothing reaches a
  consumer until a certified contract is accepted into the tier.
- `fixtures/package-contracts/implementation-census-primitive-returns` is the
  end-to-end tracer: six exports certify and five are refused by name, and
  `fixtures/reactive-ir/package-plain-return-consumer` is the consumer's half
  (coverage: 82 projects, 441 findings; the stable-main ledger moves to 137).
  The census premises one by one are pinned on synthesized transcripts in
  `type_facts.rs`, the veto against real JavaScript in
  `synthesized_vetoes_tests.rs`, and the walk in `returns_walk.rs`.
- Measured on `make contract-coverage-census`
  (`phase22/2026-09-23-what-holds-an-import-open.md` § After ADR 0113): the
  consumer view's "nothing open, a callable" goes 0 → **161**, exactly the six
  exports above, and `returns` "never proposed" 887 → 309. 302 sites now carry a
  named withdrawn operation, and 112 (`@kobalte/utils` `callHandler`, a
  TypeScript-source case the probe harness cannot load) a veto that did not
  complete.
- **The census gate fails, and not on a claim.** Degenerate sites 127 → 151
  against the pin: five `@solid-primitives/rootless` and `trigger` exports lose
  a `reads` closure to `no recipe in corpus`. Their artifact cases carry
  `@solid-primitives/utils`' accepted contract digest, which this ADR changes,
  so their ten hand recipes stop addressing. All five were open at every import
  before and after. The pin is not moved here; re-addressing those recipes is
  the scaffold's two-pass review, and so is every later change to a dependency's
  certified contract.
