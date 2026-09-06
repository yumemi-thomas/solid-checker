# ADR 0038: The `creates` census classifies under the export's declared signature

- Status: accepted and implemented (2026-09-06); written with the
  implementation
- Date: 2026-09-06
- Owners: Type Facts producer (`apps/solid-typefacts`) and the policy-2
  `creates` census (`type_facts.rs`)
- Relation: narrows ADR 0008's uncensused-form refusal for the three
  type-decided forms (coercion, iteration protocol, `await`) under a premise
  the receipt records. Leaves ADR 0034's accessor decision untouched. Handshake
  protocol 21 → 22; the lever-C slice of
  `docs/package-contract-v2/accuracy-roadmap.md`.

## Context

The `creates` census refuses an export on the first invoking form it cannot
disposition, and the producer records a coercion whenever an operand is not
provably a non-object, an iteration whenever the iterated type does not name a
reviewed engine container, an `await` whenever the awaited type is not a
primitive or a default-library `Promise`. Those classifiers are right: `any`
may be an object with a `valueOf`, and "the checker could not tell" must never
read as "no code runs here".

But the census walks **compiled, untyped JavaScript**, where every parameter is
`any`. On the 2026-09-06 corpus that one fact accounted for 357 of the 992
`censusRefused` withholdings (coercion at 60 distinct sites, 42 of them in
`motion-dom` and `motion-utils`) and 97 more (iteration at 12 sites): `p >= 1`
in `anticipate(p)`, `v > max` in `clamp`, `axis.max - axis.min` in
`calcLength`, `seconds * 1000`, `to - from`, `repeat % 2` — bodies whose every
operand the package's own `.d.ts` declares `number`.

The declared signature is not a new premise in this transaction. It is the
signature the consumer's compiler holds every argument to, and it is the
signature the synthesized veto (ADR 0036) already samples the export's
arguments from: the mandatory falsifier of the same closure is run only on
values the declaration admits. A census that refuses under `any` while the
veto observes under `number` is stricter than its own falsifier about the very
same claim, for no gain in soundness: a caller that hands `clamp` an object is
a caller `tsc` already rejects, or one whose `any` the type system already
declined to reason about.

## Decision

**The uncensused-form census of an export's root implementation is classified
with each parameter bound to the type the export's declared call signature
gives that position, and the premise is stated on the transcript, bound by the
verifier to that signature, and recorded in the receipt.**

### How the producer establishes the premise

The producer does not re-type the body itself. It builds a **twin** of the
implementation's file: the same text with one JSDoc comment inserted before
the declaration — `/** @type {typeof import("<declaration module>").<name>}
*/` — which is exactly how a JavaScript author states the same fact. The
declaration module is the `.d.ts` the export-value transcript resolved the
signature from, spelled by its own path with the declaration extension mapped
to the runtime spelling the compiler maps back (`.d.ts` → none, `.d.mts` →
`.mjs`, `.d.cts` → `.cjs`), and `<name>` is found by identity through that
module's export table, never by the symbol's own name. The twin is re-parsed
into a program that shares every other source file with the accepted program,
and the compiler's own contextual typing carries the declared types into the
body: through the parameters, through the return type into a returned arrow's
parameters (`mirrorEasing = (easing) => (p) => …`), through a declared array's
element type into a `map` callback.

The form census then runs over the twin's implementation node with the twin's
checker (`formChecker`), and every location it reports is mapped back to the
original bytes by the single insertion's length. The calls census, the
parameter-use census and the control-flow census are untouched: they run over
the accepted program, and a call is dispositioned by its callee exactly as
before.

**What binds the twin to the declaration rather than to the producer's
spelling of it.** After the twin is checked, every parameter's type on the twin
must print byte-identically to the declared signature's, taken from the
*accepted* program's checker, carry the same type flags, and — for a class,
interface or enum, and for the alias a type was written through — resolve to
the same declaration node (an anonymous function or literal type carries a
fresh symbol per spelling, so its text and its alias are what compare it). The
flags and the alias are not redundant with the text: the compiler prints an
*unresolved* reference by the name it was written under, so a spelled
`@param {EasingFunction}` the twin cannot resolve prints exactly like the
declaration's `EasingFunction`, and only its error flags and missing alias
tell the two apart. A spelled twin's return type is held to the same comparison,
because a wrong spelling there would type a returned arrow's parameters under
something the declaration never said; the `import()` twin's return type is the
declaration's own by identity and is not compared — the compiler answers a
function declaration's *inferred* return type, which prints structurally where
the declaration prints an alias such as `EasingFunction`. A twin that fails either
comparison is discarded — a spelled twin falls back to the `import()` twin, an
`import()` twin leaves the census over the parameters' own types, the strictly
more refusing reading, with the refusal stated (`parameterPremiseRefusal`) for
measurement.

**What the twin costs, and the two spellings.** An `import()` type is a
module reference the original file did not have, so the compiler cannot splice
such a twin into the accepted program: it rebuilds one, re-resolving every
module of every file — about 15 ms on a 300-file program against 50 µs for a
splice — and on the corpus that moved the wall time by roughly a third, past
the 150 s budget. The producer therefore tries a **spelled** annotation first:
one `@param {<printed type>} <name>` per parameter and a `@returns {<printed
type>}`, every type printed by the accepted checker. A spelling adds no module
reference, so the twin is spliced; it resolves only when every name it uses is
global (a primitive, a literal union, a `lib` interface, a structural
literal), and the falsifier below decides whether it did. Only when the
spelled twin fails does the producer build the `import()` twin, which names the
declaration by identity, through a host (`premiseHost`) that hands the rebuild
the accepted program's parsed and bound source-file objects for every path but
the twin's. Either way the twin is released, and the per-file memo it
populated dropped, as soon as its census has run, so no twin node outlives the
transcript. Every proof
family of an export demands the same implementation transcript — up to nine
times per artifact case — so the premised census is memoized per generation
by the implementation's span and the exact annotation (`premiseCensuses`);
the transcript's other censuses were already recomputed per demand, and the
twin is the one part worth not paying nine times.

**When the twin is built.** Only when the census over the parameters' own
types recorded a form a type can clear — a coercion, an iteration, an `await` —
and only for a JavaScript implementation (`.js`/`.mjs`/`.cjs`/`.jsx`) whose
declared signature lives in a declaration file. Most bodies record no such
form and pay nothing; a TypeScript source artifact's parameter types are
already the checker's; a body whose declared types are all `any` binds nothing
worth stating.

**What the producer refuses to premise**, each by name:

- a declaration with a rest parameter, or an implementation whose arity
  differs from the declaration's;
- an implementation that is neither a function declaration nor the sole
  initializer of a variable statement — a method, a property assignment, a
  declarator list — because the compiler reads a `@type` tag on those as
  something else or not at all;
- a declaration that already carries a JSDoc tag stating a type — a braced
  type on `@type`, `@param`, `@returns`, `@typedef`, `@satisfies` or
  `@callback`, or any `@template`, `@this` or `@overload`: two tags would
  compete, and which one the checker honours is not a premise this ADR states.
  A description-only tag (`@param seconds - Time in seconds.`, the shape
  bundled output keeps from the authors' prose) states no type and does not
  block;
- an export name that is not an identifier, or a declaration module whose
  path cannot be spelled in an `import()` type;
- an export with an overload set: the premise binds exactly one declared
  signature.

### What the transcript states

`ExportImplementationTranscript.parameterPremises` (handshake protocol 22):
one entry per parameter, in position order, each `{index, type}` where `type`
is the declared type's printed form — byte-identical to the export
transcript's `callSignature.parameters[i].value.type.text`, because both come
from the same printer over the same type. The field is stated on the export's
root implementation only. An empty form list on a premised transcript means
"no form under the declared signature", which a protocol-21 consumer would
read as "no form at all", so the handshake moves although the field is
additive.

### What the verifier does

`census_root_premises` runs before any form of the root is read:

- a transcript with no premise passes untouched — its forms were classified
  over the parameters' own types, the strictly more refusing reading;
- a premise is admitted only against the export's **one** declared call
  signature (`stated_call_signatures`, the same set the veto is synthesized
  from), with no rest parameter, exactly one entry per declared parameter, in
  position order, each `type` equal to that parameter's stated type text;
  anything else refuses by name and records nothing;
- every admitted entry becomes a `census-premise:<path>:<start>:<end>:<i>:<type>`
  witness site, so the receipt states the condition the closure holds under;
- a premise stated on a **local declaration's** transcript (depth > 0) refuses
  the census: a helper has no declared signature to bind.

### What `creates: []` now means

Under this ADR, `creates: []` for an export whose receipt carries
`census-premise:` sites means: this invocation of this export, in this
artifact case, under this guard, **called with arguments of the declared
parameter types**, registers no version-1 resource into a runtime outside the
invocation. That is the condition the mandatory veto already ran under. A call
site whose argument is `any`, or a caller that bypasses the declaration, is
outside the premise — exactly as it is outside every Type Facts claim.

## Alternatives considered

- **Re-type the body inside the producer** — bind parameter symbols to the
  declared types and re-derive operand types. Rejected: it reimplements the
  checker's inference (call return types of local helpers, contextual arrow
  parameters, narrowing), and every divergence from the compiler would be a
  premise nobody reviewed. The twin lets the compiler do exactly the inference
  it does for a JavaScript author who wrote the same tag.
- **Spell the declared parameter types only** (`@param {number} min`) from
  `TypeToString`. A printed type is a second spelling that can fail to resolve
  or resolve to something else; on its own it would lose every parameter typed
  by a `.d.ts` name. It is kept as the *first attempt* because it is spliced
  rather than rebuilt, with the falsifier deciding, and `typeof import(…).name`
  — which names the declaration by identity — as the fallback. The spelling is
  also the natural mechanism for the helper case below, where no declaration
  exists.
- **Treat declared types as evidence for accessors too.** Rejected, as ADR 0034
  rejected it: a declared property type says nothing about whether the member
  is a getter. Accessor forms are classified exactly as before; in the twin a
  `.d.ts` property still resolves to a declaration-file node, which is not
  runtime bytes.
- **Leave the refusal.** Sound and simplest, but it refuses a claim the
  transaction's own falsifier already tests under the declared types, and it is
  the single largest census refusal class on the corpus.

## What still refuses

- **A local helper's parameters.** `applyPointDelta` clears its own `+ translate`
  under the premise and then refuses at depth 1 on `scalePoint`'s
  `point - originPoint`, whose parameters have no declaration. The follow-up is
  to carry the call-site argument types from the caller's twin as the callee's
  premises (`@param` per slot, spelled from the caller twin's checker), stated
  on the local-declaration demand and bound the same way. Pinned by
  `helperCoercion`.
- **`unknown`, a bare type parameter, an object type** as an operand: not
  provably a non-object under any premise. Pinned by `untypedCoercion`.
- **A structural iterable** (`Iterable<number>`) as a spread or `for…of`
  operand: its iterator is the caller's. Pinned by the census fixture's
  `spreadUntyped`, whose declaration is `Iterable<number>` and which refuses
  under the premise exactly as it did over `any`.
- **Everything the producer refuses to premise** (above), and every body whose
  first refusal is not a type-decided form — an accessor on a non-parameter
  receiver, a write into a parameter's object, `instanceof`.
- **The default-library accessor premise now reaches JavaScript bodies.** A
  parameter declared `PointerEvent` makes `event.button` resolve to a
  `lib.dom` property signature, which `accessorKindForSymbolLocked` has always
  read as a data property ("the default library describes the engine"). Before
  this ADR such a read on an untyped parameter was an unknown accessor
  dispositioned `parameter-rooted-accessor`; now it records no form. The
  premise is not new — it is the one every TypeScript source artifact has
  always been censused under — but it is newly common, and a page that
  redefines a host prototype is outside it, as it always was.

## Consequences

- Protocol 21 → 22; schema digest and the Rust client move with it.
- `implementation-census-creates` gains five exports: `typedCoercion`,
  `returnedCallbackCoercion` and `declaredMemberCoercion` certify with
  `census-premise:` sites; `untypedCoercion` and `helperCoercion` refuse.
  The generated-census tracer pins 14 `creates` closures and 18 withheld.
- Producer tests: `TestDeclaredSignaturePremise*` in
  `declared_signature_premise_test.go` — a declared `number` clears the
  coercion; `unknown` and `Iterable<number>` keep refusing under a bound
  premise; a returned arrow's parameter and a declared member's type are
  carried; an existing JSDoc tag and an arity mismatch refuse the premise by
  name; every reported location names the original bytes.
- Verifier test: `creates_census_binds_a_declared_signature_premise_to_the_stated_signature`.
- Measured on the 2026-09-06 ecosystem corpus: withheld `creates` candidates
  1093 → 904, `censusRefused` 992 → 786, coercion refusals 357 → 80 (60 → 15
  distinct sites), iteration 97 → 90, accessor refusals 348 → 398 (unhidden
  by the cleared coercions); 368 certified / 30 refused unchanged. CPU
  accounting shows parity with the previous binaries; the benchmark report was
  not repinned because the machine ran the *previous* binaries at 155–165 s
  that day against a 117 s pin (`docs/precision-backlog.md`).
