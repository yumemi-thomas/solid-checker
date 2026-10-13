# ADR 0167: A global function called by name is a reviewed default-library call

- Status: accepted and implemented (2026-09-30); written with the implementation
- Date: 2026-09-30
- Owners: the Type Facts producer's return-site evidence
  (`primitive_syntax.go` `defaultLibraryCallLocked`,
  `defaultLibraryGlobalCallLocked`), and the plain `returns` census's evidence
  (`type_facts.rs` `DEFAULT_LIBRARY_GLOBAL_CALL_RETURNS`,
  `reviewed_default_library_call_return`, `plain_return_evidence`)
- Relation: widens where ADR 0113's amendment (2026-09-28, handshake protocol
  67) finds the evidence a `plain` return rests on beside its type. Nothing
  else in ADR 0113, ADR 0149 or ADR 0103 moves. No handshake protocol: the wire
  field is unchanged and a consumer that does not know the new spelling
  refuses it.

## Context

`docs/package-contract-v2/phase22/2026-09-30-primitives-last-blockers.md`
ranks what is the last open cause of an export at the @solid-primitives
checkpoint. One of the few that is neither an owner decision nor a value shape
no census decides is `@solid-primitives/utils`' `number`:

```js
const number = (raw) => Number(raw);
```

It is the *only* open cause of `number` in every host. The census refuses it with
the amendment's own words: the value is typed a primitive, but in a JavaScript file
that type may be only a reassignable binding's declaration, and the value is
not a primitive by its grammar, nor what a reviewed default-library member
returns.

The third fact was there for `Math.min(a, b)` and never for `Number(raw)`. The
producer states a default-library call only when the callee is a member read of
an identifier (`Receiver.member`), so a global function called by its own name
stated nothing, however exactly the checker resolved it.

## Decision

### The producer states the name (no protocol)

`defaultLibraryCallLocked` also answers a returned call, after
identity-preserving wrappers -- not optional, not `new`, not tagged -- whose
callee is an identifier that resolves, by symbol, to declarations in the
default library alone, and which the file neither writes nor deletes
(`immutableAliasLibrarySourceIsStable`, with no receiver). The answer is the
name, `Number`, with no dot. A binding that only shares the spelling -- a
parameter, a local `const`, an import -- resolves to a symbol declared outside
the default library and names nothing; `new Number(raw)` is a `new` expression
and never reaches the function.

The producer states any such function, `setTimeout` included. What a built-in
hands back is the consumer's reviewed question, as it is for `Math.min`.

### The consumer reviews it

`DEFAULT_LIBRARY_GLOBAL_CALL_RETURNS` lists the receiverless default-library
functions whose *called* value is a primitive whatever the arguments are:

| function | value |
| --- | --- |
| `Number`, `parseFloat`, `parseInt` | a Number |
| `String` | a String |
| `Boolean`, `isNaN`, `isFinite` | a Boolean |

A conversion may run the argument's `Symbol.toPrimitive`, `valueOf` or
`toString`, or throw; none hands that object back. That reach is the
`callbacks` domain's own question, decided where it always was, and the plain
`returns` census reads only the value. `plain_return_evidence` asks the
member tables first and this one second; a member is `Container.member` and a
function has no dot, so the two never collide. The table is separate from ADR
0103's alias tables on purpose: an export that *is* `Number` names no container,
so the alias premise does not reach it, and nothing here widens what that
premise reviews.

`String(symbol)` is a String describing the symbol, `Number(1n)` is a Number,
and a Symbol handed to `Number` throws. Nothing in the
table returns a wrapper object when *called*.

### What is not decided here

The same trust as the existing rows and no more: an assignment to the global
through `globalThis` or a property descriptor is not a write the file's own
binding walk sees, exactly as for `Math.min`. The finite veto (the
synthesized `typeof` observation ADR 0113 runs) is what stands beside the
census.

## Consequences

- `fixtures/package-contracts/implementation-census-primitive-returns` gains
  `toNumber` and `toLabel` (certify, witness `default-library:Number` and
  `default-library:String`) and `shadowed` and `wrapped` (refused by name), pinned
  by `the_primitive_returns_census_certifies_exactly_the_primitive_completions`.
  The producer's facts are pinned by `TestReturnSitesStatePlainReturnEvidence`
  (a call by name, parenthesized, shadowed by a parameter and by a `const`,
  written, constructed, optional; a default-library function without a row is
  stated), and the reviewed rows and their neighbours by
  `plain_return_evidence_is_required_beside_the_type`.
- `bin/solid-typefacts` is rebuilt: the source manifest moves, so the
  certification pins do. No document's digest moves and no fixture in the
  corpus proposes a global-function return.
- Measured with `make primitives-checkpoint`'s three host runs (release binary,
  without the misuse ledger), each side with its own rebuilt
  `bin/solid-typefacts`: at 816d6f58 (ADR 0165), clean exports 99 -> 100 (none),
  99 -> 100 (browser) and 100 -> 101 (node); at d88e3b50, 90 -> 91, 90 -> 91 and
  100 -> 101. In both, `utils:number` alone moves and a full diff of the three
  measure files shows nothing else.
- `@solid-primitives/sse`' re-export of `number` does *not* follow, and it was
  first expected to. Its `returns` is held by a different cause, an attribution
  widening (`fallback-all: PackageContractExportMissing`) filed at an import in
  the sibling barrel `dist/transform.js`, which ADR 0133's rung declines because
  the binding is not in the entry's own file. It is recorded in the phase-22
  note as the next attribution gap.

## What still refuses

- `getRemSize`'s `parseFloat(getComputedStyle(...).fontSize)`: the function is
  reviewed now, but its sibling branch reads a module `let`, which is neither
  syntax nor a reviewed call.
- A regular-expression literal's `test` (`scroll`'s `isScrollable`): the
  receiver is a literal, not an identifier, so no name is stated.
- `Date.now()`, `Math.random()` and every other default-library member without
  a row in ADR 0103's tables: nothing here reviews a member.
