---
status: accepted
---

# V1 names the invoking forms the call census does not record, and reaches a module-local declaration

## Decision

The producer carries two new facts. Handshake protocol moves 13 → 14 and the
schema digest moves with it, from
`sha256:1e85e91a37409d8c4d1527ac9778155e10f6ff400cfa2c7e5dbeab0f771866ec` to
`sha256:0d246a6cf7682e3f756df3ce54569cfca2dfeb57006a3198dabad51e43f96fc4`.

**`uncensusedInvokingForms`** on `ExportImplementationTranscript`: per form,
every syntactic position in the implementation that can invoke user code and
that the call census does *not* record. **`localDeclarationLocation`** on
`ExportValueDemand`, answered by **`localDeclaration`** on
`ExportValueTranscript`: an implementation transcript for the function-like
declaration at an exact source range, reachable without any export naming it.

Neither is wired to a census. This is the producer half of
`docs/package-contract-v2/phase21/2026-09-03-implementation-census-plan.md`
§ 4.1; the census that consumes them is § 4.3 and does not exist yet.

## Why the call census is not enough

`implementationCallCensusLocked`
(`apps/solid-typefacts/internal/typefacts/tsgo/export_value_transcripts.go`)
emits an `ImplementationCall` row for `ast.IsCallExpression` or
`ast.IsNewExpression` and returns for everything else. A closed behavioral call
domain asserts a **zero upper bound** on the operations one invocation gives
rise to (`semantic-model.md` § "What a closed call domain denies"), so a census
that enumerated calls alone would certify closure for an export whose body
reaches user code through a tagged template, an accessor, or the iteration
protocol — forms no `calls` row mentions.

The fourth shared rule of that section is the constraint this field satisfies:
its non-call form lists are open-ended, and **a census may conclude closure only
where the producer classified every form it walked, refusing by name on any form
it does not classify**. A classifier whose default is "ignore" fails that
however long its list.

## The kind vocabulary, and its default

The enum is closed. An unrecognized string is not mapped to a catch-all: the
Rust `UncensusedInvokingFormKind` carries no `#[serde(other)]` arm, so
deserialization fails and the whole transcript is rejected, exactly as for an
unrecognized `CallKind`. A producer that invented a kind is a producer this side
does not understand, and reading its census as rows of unknown kind would keep
every *other* field of those rows in play.

| kind | what it names |
| --- | --- |
| `tagged-template` | a `TaggedTemplateExpression`; the tag is invoked. An `html` template counts here and nowhere else |
| `get-accessor` | a property access, element access, or destructured member resolved to a symbol with a get-accessor declaration, in a reading position |
| `set-accessor` | the same, with a set-accessor declaration, in an assignment target position |
| `property-access-unknown-accessor` | a member that resolves to **no symbol** — an `any` receiver, a computed key, an index signature — or to declarations that are not the snapshot's runtime bytes; plus object spread, JSX prop spread, and an object rest element, which read every own enumerable property of a value whose shape is not statically known |
| `decorator` | a `Decorator` application |
| `iteration-protocol` | `for…of`, a spread element, an array binding pattern and `yield*` whose operand's type does not prove the iterator is the engine's; `for await…of` and an array *assignment* pattern (`[a, b] = src`) unconditionally |
| `using-dispose` | a `using` or `await using` declaration list; scope exit reaches `Symbol.dispose`/`Symbol.asyncDispose` |
| `instanceof` | reaches `Symbol.hasInstance` on the right operand when that operand defines it |
| `await-then` | an `await` some constituent of whose operand type is neither a primitive nor a default-library `Promise` |
| `coercion` | a template expression or operator application whose operand is not provably a non-object, so evaluation may reach `Symbol.toPrimitive`/`valueOf`/`toString` |
| `jsx-element` | a JSX element, self-closing element, or fragment |
| `unclassified-invoking-form` | **the catch-all**, carrying the compiler's own node-kind name |

**The catch-all is the load-bearing row, and the classifier's default reaches
it.** Classification runs in three steps: a node below the compiler's own
`KindFirstNode` is a token and invokes nothing; a node whose kind is named above
gets that kind; a node whose kind is on `nonInvokingNodeKinds` — the reviewed
list of non-token kinds that provably cannot invoke user code by being
evaluated — gets no row; **everything else is `unclassified-invoking-form`**. A
kind a future compiler revision adds therefore refuses on arrival rather than
passing in silence, and the consumer refuses on this row unconditionally,
because there is nothing else it could soundly do with a form nobody has
classified.

The token boundary is taken from the compiler (`ast.KindFirstNode`, which today
has 166 token kinds below it) rather than restated, so a keyword a future
revision adds is covered without an edit. `nonInvokingNodeKinds` is a claim one
kind at a time — 123 of them — and it says nothing about a node's children:
`CallExpression` is listed because another census records it, and
`ObjectLiteralExpression` is listed even though `{ x: a.x }` invokes a getter,
because that getter is the child access's own row. JSDoc kinds are cleared by
prefix for the same reason the token boundary comes from the compiler.

The list is also a claim about **every position** a node of the kind can
occupy, which is why four kinds that could plausibly appear on it do not.
`ArrayLiteralExpression`, `PropertyAssignment` and `ShorthandPropertyAssignment`
are values in one position and destructuring patterns in another — the compiler
reinterprets the same nodes through `checkDestructuringAssignment` — so
`[a, b] = src` drives the iteration protocol and `({ a } = src)` performs a Get
per member, while `const o = { a }` and `[first, second]` invoke nothing. The
switch asks `ast.GetAssignmentTarget` before answering. `=` is an assignment
rather than a coercing operator, so nothing else in the classifier would have
seen either form. `YieldExpression` is off the list for the neighbouring
reason: the asterisk, not the kind, decides.

Two kinds must also be listed as tokens *nowhere*: `JsxText` and
`JsxTextAllWhiteSpaces` sit below `KindFirstNode`, so listing them would have
been dead weight reading as a reviewed claim.

Today `WithStatement` is the only node kind that can appear in a function body
and reaches the catch-all. That is a property of the current allowlist, not a
design goal: the arm exists for the kinds nobody has thought of.

`nodeKind` is schematised as `{"type": "string", "minLength": 1}` — no
`maxLength`, which is the convention every string field in
`schema/typefacts-v1.schema.json` follows (no string in the file bounds its
length; the array fields carry the `maxItems` bounds instead). The `minLength`
is the part that matters here: it is what stops the load-bearing refusal row
from arriving anonymous, and the client repeats the check because a schema is
not what the client reads.

### Two forms deliberately absent

**A `Proxy` trap is out of the producer's reach and is not faked.** A trap
belongs to the object a value happens to be at runtime, not to any syntax:
`obj.x` on a proxy is the same `PropertyAccessExpression` as `obj.x` on a plain
object, and no walk of an implementation can tell them apart. Inventing a
`proxy-trap` marker would claim a census the producer cannot perform. What the
producer *can* say is that a member it could not resolve is unresolved, which is
`property-access-unknown-accessor` — a statement about the checker's knowledge,
not about proxies. A consumer whose claim requires that no trap ran must obtain
that premise elsewhere. `for…in` is on the non-invoking list for the same
reason: on a plain object it reaches nothing, and on a proxy it reaches
`ownKeys`, which is the same limit.

**An optional call, `f?.(x)`, is already a `CallExpression`.** The call census
records it like any other call, so a marker for it would be a second row for a
form that is not missing.

### The getter limit, stated exactly

`get-accessor` and `set-accessor` are claims the checker made: the access
resolved to a symbol whose declarations include an accessor. A symbol whose
declarations include *no* accessor is a plain data property or a method, and
reading it invokes nothing — **no row**. A symbol that does not resolve is
neither, and it is recorded, because absence of a symbol is not evidence of a
plain property. Which of get and set is reported follows
`ast.GetAssignmentTarget`, the same question the parameter-use census asks; a
get-only accessor written to, and a set-only accessor read, each refuse under
the accessor that does resolve rather than modelling the asymmetry.

An element access resolves its member from the **argument expression**, and
only when that expression is an exact string or numeric literal — the arm
`checker.getSymbolAtLocation` has for `KindStringLiteral`/`KindNumericLiteral`.
Querying the access node itself resolves nothing whatever the key, so
`widget[key]` refuses even where `key`'s *type* is the literal `"value"`: a
literal type is not a literal key, and over-refusal is the safe direction.

#### The premise underneath it, and where it fails

"The declarations include no accessor, therefore reading invokes nothing" is
sound only where **the declarations inspected are the bytes that run**. A
hand-written `index.d.ts` may declare `readonly value: number` over a published
`.js` getter; the symbol resolved through that declaration carries no
`GetAccessor` node at all, so the premise would have quietly certified a getter
the census never saw. Every declaration must therefore sit in a file the
snapshot carries as runtime source — a non-declaration file of the accepted
program, exactly the set the producer publishes as `Sources()` — and a
declaration anywhere else is recorded as `property-access-unknown-accessor`.

The **default library is the one admissible exception**, and it is not a
weakening: `lib.*.d.ts` describes the engine, whose implementation is not user
code, so no `lib`-declared member can reach a user callable however the engine
implements it. Without the exception every `arr.length` would refuse and the
kind would carry no information.

Two limits remain open and neither is closed here. The premise is about
*declarations*, so a subclass that redeclares a plain member as a getter is
invisible whenever the static type names the base declaration. And a `Proxy`
trap is outside every producer census, for the reason below.

The same premise governs destructuring, in both of its spellings. A
declaration pattern's members are resolved through the pattern's own type (the
compiler answers the *source* type for a binding pattern, which is what a
member must be looked up in), and an assignment pattern's through
`GetPropertySymbolOfDestructuringAssignment` — never through
`GetTypeAtLocation` on the literal, whose expression type in that position is
the shape of the pattern rather than of the source. A **rest element** resolves
nothing by name: it reads every remaining own enumerable property and its own
identifier is the new object, not a key, so `const { ...rest } = src` refuses
even where `src` happens to carry a property called `rest`.

### The `await` limit, and the quantifier that makes it fail closed

`await-then` is recorded unless **every** constituent of the operand's type is
provably one the runtime resolves without entering user code: a primitive,
which has no `then` to call, or a default-library `Promise`, whose `then` is
the engine's own. The quantifier is per constituent for the same reason
`coercion`'s is, and "the type has no `then` member" is *not* a licence to stay
silent — a union missing `then` in one constituent carries it in another, an
unconstrained type parameter has no members the checker can enumerate, and an
index-signature type such as `Record<string, unknown>` declares no `then` while
permitting one at runtime, whose `Get` would reach a getter and whose value
`await` would call.

### The iteration limit, and the two halves of the container table

`iteration-protocol` names the syntaxes that drive `Symbol.iterator` and then
the iterator's own `next` and `return`. It is recorded unless **every**
constituent of the iterated value's type carries a `[Symbol.iterator]` that a
reviewed table of default-library interfaces vouches for — the same
per-constituent quantifier `await-then` uses, for the same reasons: a union
missing the member in one constituent carries it in another, `any` and
`unknown` and an unconstrained type parameter enumerate no members at all, and
an index-signature type declares no iterator while permitting one at runtime. A
nil `[Symbol.iterator]` lookup therefore **refuses**; "the checker could not
find it" is never "iterating this reaches no user code". One consequence is
worth naming: a non-iterable operand records a form it cannot actually reach,
because iterating a number is a `tsc` error and a runtime `TypeError`, and this
census does not trade a fail-closed quantifier for silence on code that does not
run.

**What a table row claims has two halves, and both are needed.** The
`[Symbol.iterator]` named by the declaration is the engine's own factory,
*and* the value is an object the engine itself created — so the iterator that
factory returns, and therefore that iterator's `next` and `return`, is engine
code too. The reviewed set is `Array`, `ReadonlyArray`, `String`, `IArguments`,
`Set`, `ReadonlySet`, `Map`, `ReadonlyMap` and the typed arrays. A tuple
resolves through its `Array` base and answers the same; a primitive string
clears through `String`'s apparent type, which is why the primitive shortcut
`coercion` uses is *not* reused here — a string is a primitive and is iterable.

**The absent rows are the whole precision of it.** `Iterable`,
`IterableIterator`, `IteratorObject`, `Iterator`, `ArrayIterator`,
`MapIterator`, `SetIterator`, `StringIterator`, `RegExpStringIterator`,
`SegmentIterator` and `Segments` all declare `[Symbol.iterator]` in the default
library and none of them is on the table: every one is a structural contract a
user object satisfies, so the factory named by the declaration is not the
factory that runs. `Generator` is the sharpest case and the reason to state this
rather than infer it from "declared in `lib`" — a generator's `next` runs a user
function body. This is exactly the `Promise` versus `PromiseLike` split
`await-then` draws. The forty-odd DOM and web-worker collections — `NodeList`,
`URLSearchParams`, `Headers`, `FormData` — are absent too: their iterators are
engine code in fact, but they were not reviewed, and "the browser probably owns
it" is not a premise.

**Two arms ask no type question.** `for await…of` resolves
`Symbol.asyncIterator` first — declared in the default library only by
`AsyncIterable`, `AsyncIterableIterator`, `AsyncGenerator` and
`AsyncIteratorObject`, every one a structural contract whose `next` is a user
body — and when the value carries none of them it falls back to the sync
protocol and `await`s each result, invoking whatever `then` those values carry.
Neither half has an engine-owned case worth a row. An array *assignment*
pattern has no operand to ask about: the iterated value is the assignment's
right-hand side, or, inside `for ([a] of pairs)`, the element type of another
node's iteration, and `GetTypeAtLocation` on the literal answers with the shape
of the *pattern* rather than of the source — the same trap the object
assignment-pattern arm documents. Deriving the source there is its own premise
and is not taken.

**The limits are the ones every declaration-based premise in this census
carries**, and they are recorded rather than closed. A value whose static type
is `Array<T>` while the runtime object is a subclass overriding
`[Symbol.iterator]` answers from the base declaration; a constrained type
parameter clears through its constraint's apparent type exactly as
`await value` does when `T extends Promise<number>`; and a `Proxy` is outside
every producer census for the reason stated above.

**The lookup key comes from the compiler, not from a spelling.** A
well-known-symbol member is stored under a name the compiler derives from the
program's own `SymbolConstructor` declaration when it has one, falling back to
`__@iterator` only without it, so the key is asked of
`getPropertyNameForKnownSymbolName` — the same call the compiler's own iterable
resolver makes. Spelling it would silently find nothing, which in a fail-closed
census reads as a refusal rather than as an error. An **optional**
`[Symbol.iterator]?` refuses, matching that resolver, which requires the member
to be non-optional before reading the protocol off it.

**This narrowing moves no field and no protocol.** The field's meaning is
unchanged — a producer at or above the protocol that introduced it classified
every form it walked — and no schema shape changed, so
`TYPE_FACTS_HANDSHAKE_PROTOCOL` stays at 15 and the schema digest stays put.
What discriminates a narrowed producer from an unnarrowed one is producer
*identity*: the source-manifest digest and the build id, which the handshake
compares field-for-field and which move on their own. A protocol bump here
would assert a wire break that did not happen and would force every pinned
consumer and fixture to migrate for nothing. The protocol number could not
protect against a wrong premise anyway; the premise is what the tests pin.

## Absence is not a guarantee — and how a consumer tells

**A present empty list and an absent field are different facts.** A present
empty list is the producer's positive claim: every form it walked was a call, a
construction, or a kind on its reviewed non-invoking list. An absent field is a
producer that never classified anything, and a census must refuse.

Neither serde nor the CBOR decoder can tell them apart: the field is
`#[serde(default)]` on the Rust side and `omitempty` on the Go side, so an
older producer's silence decodes as an empty list, and a nil Go slice and an
empty one are the same bytes. **The discriminator is therefore the handshake
protocol, and only the handshake protocol.** `TYPE_FACTS_HANDSHAKE_PROTOCOL`
and `TypeFactsHandshakeProtocol` move to 14 together, `protocolv3_test.go`
pins the number, and the certification identity check
(`contract_certification/type_facts.rs`) compares protocol, schema digest, and
build id field-for-field before any transcript is read. A census must establish
the protocol; it must never conclude the guarantee from the list's emptiness.

This is why the change is a protocol break rather than a merely additive field.
In the other direction the break is the ordinary one: `ExportImplementationTranscript`
denies unknown fields, so a protocol-13 consumer rejects a protocol-14
transcript outright.

## `complete` is unchanged, and says nothing about this

`ExportImplementationTranscript.complete` keeps exactly its current meaning: the
conjunction of seven gates the producer clears in order, each of which otherwise
appends its own open reason and returns —

1. the queried node is an **exact identifier** (`identifierNotExact`);
2. `GetSymbolAtLocation` **resolves** it (`symbolUnresolved`);
3. the alias chain resolves to a **canonical target** (`aliasUnresolved`);
4. the value's type has **exactly one call signature**
   (`callSignatureNotUnique`);
5. the selected signature has an implementation declaration **with a body**
   (`implementationUnavailable`);
6. that implementation has a **resolved declaration**
   (`declarationUnavailable`);
7. the control-flow census has **no unsupported branch**
   (`controlFlowUnsupported`).

(`sourceUnavailable` precedes all seven: without the source file there is no
node to query.)

**`complete` says nothing about the calls census being total.** Gate 7 is about
control flow, not invoking forms, and `calls` records `CallExpression` and
`NewExpression` only, so a transcript can be `complete: true` while a tagged
template in its body invokes code no `calls` row mentions. The enumeration
guarantee lives entirely in `uncensusedInvokingForms`. § 4.1 of the census plan
proposed relaxing the field's *name* to spell that out; the name is left alone
here, because renaming a field every consumer reads is a larger break than the
one this slice takes, and the two doc comments plus this section state the
distinction where a consumer will meet it. The plan's item is therefore
partially discharged: the guarantee has its own field, and `complete` has not
been renamed.

## Reachability, and the one place this census differed from the call census

Rows come from `walkImplementationBodyLocked`, the same walk the call census and
the parameter-use census share, so the three never disagree about which callable
frame a position sits in or whether invoking the export reaches it. A form
inside a nested callable carries `enclosingCallable` and `captured`, with the
same discipline and for the same reason: lexical containment in a returned
closure is not execution.

What this census did **not** share was the call census's jump withholding. There,
a call inside a region a `break` makes non-universal was dropped so an
over-optimistic positive `Reach` never reached the wire. Here a dropped row is
**silence**, which is the exact failure this field exists to prevent, and an
over-optimistic `Reach` on a marker can only make a consumer refuse a form that
might not have run. Over-refusal is the safe direction; silence is not.

**Amended 2026-09-04, protocol 15: the call census no longer withholds either,
and for exactly the reason stated above.** See the amendment at the end of this
document. The asymmetry that remains is only in the parameter-use census, which
still drops those rows.

## The local-declaration demand

`localDeclarationLocation` names a **declaration node** by its exact source
range. A census recurses into same-snapshot callees, and most of them are
module-local functions: `implementationLocation` cannot reach one, because it
starts from an identifier, resolves its symbol, and takes the implementation of
its single call signature — machinery that presupposes a binding some export
names.

Refusals are by open reason, never by answering about a different declaration:

| open reason | when |
| --- | --- |
| `sourceUnavailable` | the accepted program resolved no file at that path, or the byte range is outside it or off a UTF-8 boundary. This is a statement about the *program* |
| `declarationOutsideSnapshot` | the file is in the program but carries no runtime bytes, i.e. it is a declaration file. Every `lib.*.d.ts` and every dependency `.d.ts` is a program file, so `sourceUnavailable` alone does not separate snapshot source from a description of code |
| `declarationNotExact` | no node in that file has exactly this span, or the node that does is not function-like. **Containment is not a match** |
| `declarationAmbiguous` | more than one function-like node has exactly this span |
| `implementationUnavailable` | the declaration has no body |
| `symbolUnresolved` | an anonymous callable whose name cannot be recovered from an enclosing variable declaration |
| `declarationIdentityUnbound` | the declaration the checker resolves from the located node's own symbol does not sit inside the demanded span. An internal-consistency refusal; see below |
| `declarationUnavailable`, `callSignatureNotUnique`, `controlFlowUnsupported` | as for the export path |

### What binds the answer to the demand, and what does not

An earlier form of this decision claimed identity was "bound both ways" by the
answer's `location`. **It was not.** The producer copies the demanded location
into that field verbatim, and the client compared only that field, so the
comparison was a tautology: a producer answering about a different helper would
have echoed the demand exactly as one answering about the right helper does.
What is actually bound, and all that is:

1. **Presence agrees with the demand.** `Session::export_values` refuses a
   `localDeclaration` for a demand that asked for none, and refuses its absence
   for a demand that asked for one.
2. **The echo matches.** Cheap, and it does catch one real confusion — an
   answer built for a *different demand of the same batch*, whose location
   differs.
3. **The resolved declaration lies inside the demanded span.** This is the half
   the producer does not echo: `Declaration.location` comes from the located
   declaration's own name node through `resolvedDeclaration`. It is a
   containment rather than an equality because a named function resolves to its
   *identifier* while an anonymous `const helper = () => …` resolves to the
   arrow itself. Where `queryName` and that declaration's `name` are both
   populated they must agree too. The producer refuses its own violation of
   this with `declarationIdentityUnbound`; the client refuses the answer.

The location is also hashed into the export-value demand digest, so two demands
differing only in which helper they name are two different demands and neither
answer can be replayed as the other.

None of this makes a well-formed transcript about some other declaration
impossible to construct: the producer is trusted for the *contents* of a body it
censuses, here exactly as it is for an export's. What is refusable without
reading the source is an answer whose two identity fields disagree with each
other, or which describes a declaration outside the bytes that were asked
about.

The declaration's name is taken from the declaration when it has one and from an
enclosing `VariableDeclaration` otherwise, which is the one indirection taken —
`const helper = () => …` carries its name there. Its signature comes from
`GetSignatureFromDeclaration` rather than from a type's call-signature list,
because the demand already names the declaration; nothing about overload
selection is being claimed.

## What is pinned where

- Every kind, and the negative controls that must produce no row, against the
  compiler in
  `apps/solid-typefacts/internal/typefacts/tsgo/uncensused_invoking_forms_test.go`.
  The negatives are load-bearing: `awaitPromiseForm`, `stringTemplateForm`, and
  `plainMemberForm` contain the *syntax* of a classified form and must stay
  silent, because the classification is decided by the type rather than the
  spelling.
- Every classifier *arm* whose answer depends on position or on the checker's
  resolution, in the same file's `branchSource`: both destructuring spellings
  against their value-position controls, the get-only-written and set-only-read
  asymmetries, element access with and without a literal key, the coercion
  operators, the four `await` cases the quantifier decides, the
  runtime-bytes premise against a `.d.ts` declaration, and the default-library
  exception. Several of those exports are deliberately TypeScript errors,
  because the arms exist for code the checker still resolves.
- The wire round trip through the real producer process — CBOR, the closed kind
  enum, the client's validation, and the local-declaration identity binding — in
  `rust/crates/typefacts/tests/session_process.rs`. The client-side defenses
  themselves — both `validate_implementation_transcript` arms, all three
  local-declaration presence/location arms, the resolved-declaration
  containment, and the closed enum rejecting an unrecognized `kind` by failing
  the *whole transcript* — are unit tests in `rust/crates/typefacts/src/session.rs`
  and need no producer.
- The classifier over published `.js` bytes, through ordinary contract
  generation, in `fixtures/package-contracts/uncensused-invoking-forms`. Its
  `expected.json` and `expected-proposal.json` carry no marker rows —
  `uncensusedInvokingForms` is a transcript field, not a contract field — so
  **those two files pin only that the classifier runs over these shapes without
  changing the contract.** The fixture's marker kinds are made load-bearing
  separately, by `TestUncensusedInvokingFormsClassifyThePublishedFixtureBytes`,
  which reads its `index.js` off disk and pins the kinds per export; without
  that test the fixture's README table would be prose nothing checks. Its README
  says which four kinds cannot live in a published module and why.

## Consequences

- No consumer reads either field yet, so nothing in the analyzer's output
  moves. Coverage stayed at 94 projects and 546 findings, and no existing
  contract-corpus snapshot moved.
- The census of § 4.3 can now be written against a producer that refuses by
  name. Until it is, `require_census_decides_closure` keeps refusing every
  behavioral call domain, which is unchanged.
- `docs/package-contract-v2/phase19/proof-demand-authority-audit.json` gains
  the row § 6 of the census plan drafted, and
  `scripts/package-contract-phase19.test.mjs`'s counts move with it.

---

# Amendment, protocol 15 (2026-09-04): the call census states what a jump region hides, and control-flow incompleteness carries a class

Handshake protocol moves 14 → 15 and the schema digest moves with it, from
`sha256:0d246a6cf7682e3f756df3ce54569cfca2dfeb57006a3198dabad51e43f96fc4` to
`sha256:319b22f36abf190c43ed4889bd2e5b43a93c5c1c182be8f86316c0424b73a8bc`.

## Why this is the same decision as the one above

The section "Reachability, and the one place this census differs from the call
census" drew the distinction and then left the call census alone: *there* a
dropped row was called the safe direction, *here* it was called the exact
failure. The two claims cannot both be right for the same consumer. A closed
behavioral call domain asserts a **zero upper bound** on the operations one
invocation gives rise to, and for that claim a `calls` row's absence is
indistinguishable from the call's absence — the same silence
`uncensusedInvokingForms` exists to prevent, arriving through the other census.

Measured: `switch (kind) { case "mount": render(App, el); break; }` published
**nothing** about `render`. The row was dropped because it lay in the region the
`break` makes non-universal, and because the dropped call is a `CallExpression`
the uncensused-form classifier records it nowhere either. The only trace was the
enclosing construct's `switchReachability` marker in the control-flow census,
which is why the consumer census had to refuse every such marker — and therefore
refused essentially every real function body, since the marker covers every
loop, `switch` and `try` (`docs/adr/0008-implementation-census-for-creates.md`
item 0).

## Decision 1: a row in a jump region is stated with `reach: unknown`

`implementationCallCensusLocked` no longer returns early for a location
`locationWithheldByJump` covers. It emits the row and sets `Reach` to `unknown`.

**Why `unknown` is the sound value.** Reachability here is ordered by the
strength of the positive claim it licenses: `reachable` says invoking the
implementation runs the call on every path through the frame, `unknown` says it
may run it, `unreachable` says it cannot. A jump falsifies only the **first** of
those. So `unknown` is exactly what the producer still knows, and it is the
weakest non-negative value — no consumer can read more out of it than the jump
left standing. A consumer needing a guarantee refuses it; a consumer at the
may-execute floor admits it, and a may-execute floor is the only thing a claim
about *which* callables a body can reach could ever be built on.

**Why the old drop was chosen, and why it is no longer needed.** Dropping kept
the same over-optimistic `reachable` off the wire, and for a claim that some
behavior *happens* a missing row is the safe direction: absence lends authority
to nothing. That reasoning is intact and the drop is still unnecessary, because
stating `unknown` is *strictly weaker* than stating the row that was withheld.
Nothing that was sound became unsound; what changed is that the enumeration a
negative census needs is now on the wire.

An already-`unreachable` row is left alone. The walk did not decide it from the
jump — its `unreachable` claims come only from sequential abrupt completion, a
literal-condition branch, or an unreachable enclosing construct, none of which a
labelled jump falsifies — and downgrading it would discard a proof for nothing.

Regions are keyed by **flow owner**, which is what makes this cover a jump
inside a nested callable: `controlFlowCensusLocked` never enters one and so
leaves no marker there, while `walkImplementationBodyLocked` does, and reduces
that callable's own rows. A jump whose target is not inside the frame at all has
no boundary to narrow to, so its region is the whole flow owner.

**The parameter-use census still drops the same positions**, and that asymmetry
is deliberate rather than an oversight: a use census answers positive escape
questions, where absence is no claim, and no consumer builds a negative claim on
it. Pinned in `invoking_positions_test.go`, which now asserts the call row's
presence and the use row's absence side by side.

## Decision 2: `controlFlowCensus.incompleteness` classifies each construct

`unsupported` absorbed three different situations under four marker strings: a
construct whose *reachability* the census does not model, a jump whose target it
cannot resolve, and — because the first covers every loop, `switch` and `try` —
essentially every real body. A consumer could only refuse all of them.

The new field is one row per construct, carrying the same `marker`, the
construct's `location`, and a `class` from a **closed** two-value enum. An
unrecognized string fails deserialization and rejects the whole transcript,
exactly as for `UncensusedInvokingFormKind`; reading an unknown class as either
arm picks the unsound one half the time.

| class | what it claims | produced by |
| --- | --- | --- |
| `reachability-lower-bound` | The construct was walked in full. Every site inside it is recorded by the shared body walk, and none is called `unreachable` on its account. What is missing is only the **lower** bound: control may not enter a loop body, a `catch` clause, or a selected clause. | `iterationReachability`, `switchReachability`, `tryReachability` |
| `flow-unaccounted` | The census cannot account for the construct's flow in either direction, so neither a may-execute nor a guarantee question is answered. | `jumpReachability`, **and the classifier's default** |

`unsupported` keeps its exact meaning and its consumers: any entry still opens
the transcript with `controlFlowUnsupported`.

**Why `jumpReachability` is the unaccounted class rather than the lower-bound
one.** It is emitted exactly when `jumpHandledByFallthroughConstruct` is false —
a labelled jump past an enclosing construct, a jump across a `try`, a `break` to
a plain labelled block. The **target** is what bounds every repair either census
applies to a jump: the region `implementationCallCensusLocked` reduces to
`unknown`, and the fallthrough question `constructCompletesNormallyLocked`
answers. With no target an enclosing construct of the frame owns, the producer is
not claiming to know where control goes, and it says so rather than offering a
class a consumer would admit. Over-refusal is the safe direction, and the two
shapes real code is made of — a `break` inside the `switch` or the loop that owns
it — resolve their targets and leave the construct's own lower-bound marker
alone.

**The default is the refusing arm**, which is the same discipline
`unclassified-invoking-form` follows: a marker a future revision adds without a
reviewed class arrives as `flow-unaccounted`, so it refuses on arrival instead of
passing as the admissible one.

## Why the protocol moves rather than the fields being additive

An **absent** `incompleteness` beside a nonempty `unsupported` is a producer with
no classification at all; a **present empty** one is the claim that nothing is
unmodelled. Serde cannot separate them, because the field defaults to empty, and
a nil Go slice and an empty one are the same bytes. A protocol-14 producer's
silence would read to a protocol-15 consumer as the admissible arm — the unsound
direction — and that same producer is *also* still dropping rows, which the
consumer cannot detect at all. So the handshake is the discriminator, and it is
the discriminator for both halves of this change at once.

**How the client tells the two apart, exactly.** It does not inspect the
transcript. `Session::open` refuses any producer whose handshake protocol,
schema digest, or build id differs from the client's own
(`TYPE_FACTS_HANDSHAKE_PROTOCOL`, `TYPE_FACTS_SCHEMA_SHA256`,
`TYPE_FACTS_BUILD_ID`), and certification's identity check compares all three
field-for-field before a transcript is read. On top of that, the census names its
own dependency as a constant — `CENSUS_CONTROL_FLOW_CLASSES_PROTOCOL = 15`,
beside the existing `CENSUS_UNCENSUSED_FORMS_PROTOCOL = 14` — and refuses the
demand by name when the build speaks less, so the premise is stated where it is
relied on rather than left to the handshake. In the other direction a
protocol-14 consumer rejects a protocol-15 census outright: `ControlFlowCensus`
denies unknown fields.

## One invariant the client checks itself

`validate_control_flow_incompleteness` refuses a response whose two lists name
different markers: a marker in `unsupported` with no classified construct is an
*unclassified* incompleteness — the thing the class exists to make impossible —
and a classified row for a marker the census does not report is the same
disagreement from the other side. The comparison is over **sets**, because one
marker legitimately has many rows (two loops are one marker and two constructs).
It says nothing about which class is admissible; that is each consumer's
decision, and it depends on what the consumer is asking.

## One gap this closed on the way

Admitting `tryReachability` made a pre-existing hole reachable:
`walkImplementationBodyLocked` visited a `try`'s catch clause **block** and not
its variable declaration, so a call in a destructuring catch default —
`catch ({ message = describe() })` — sat in no census at all, neither a `calls`
row nor an uncensused form. It went unnoticed because the one census that needs a
total enumeration refused every `try` outright. The walk now visits the catch
parameter first, at the clause's own reachability.

## What is pinned where

- Each class against the compiler, with its construct's exact location, and the
  call row inside it, in
  `apps/solid-typefacts/internal/typefacts/tsgo/export_value_transcripts_test.go`
  (`TestControlFlowIncompletenessClassifiesEachConstruct`). A body with two
  loops and a `switch` pins that the two lists agree as *sets* and that the rows
  are in source order
  (`TestEveryUnsupportedMarkerCarriesAClassifiedConstruct`). The catch-parameter
  gap has its own test.
- The stated row and the still-dropped use row, for all seven jump shapes the
  older tests covered, in `invoking_positions_test.go`.
- The wire round trip through the real producer — CBOR, the closed class enum,
  the client's set comparison, and the `unknown` reach of a row a jump region
  covers — in `rust/crates/typefacts/tests/session_process.rs`
  (`export_value_transcripts_classify_control_flow_incompleteness`), against two
  new exports of the `uncensused-invoking-forms` testdata project. The client's
  own check needs no producer and is a unit test in `session.rs`.
- The consumer half, and what it does with each class, in
  `docs/adr/0008-implementation-census-for-creates.md` item 0 and
  `fixtures/package-contracts/implementation-census-creates`.

## Consequences

- Coverage stayed at 94 projects and 546 findings; the ownership gate stayed at
  289 cases and 465 ledger rows. One contract-corpus fixture's snapshots moved,
  and only because the fixture's own bytes changed.
- ADR 0008 item 0 is discharged for the reason it was recorded: the census's
  premise is now a row it can disposition rather than a marker it must refuse.
  What it is *not* is a general relaxation — `flow-unaccounted` still refuses,
  and the fixture carries a negative control for it.
