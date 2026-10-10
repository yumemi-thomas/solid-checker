# Invoking forms the implementation call census does not record

`implementationCallCensusLocked` emits an `ImplementationCall` row for
`CallExpression` and `NewExpression` and for nothing else. Every other way a
JavaScript expression reaches user code — a tagged template, an accessor behind
a property access, the iteration protocol, `Symbol.hasInstance`, a thenable's
`then`, a coercion — appears in `ExportImplementationTranscript`'s
`uncensusedInvokingForms` instead, so a census that must enumerate every
invoking form refuses **by name** rather than concluding from silence.

This fixture is the real-package half of that census: the producer classifies
these exports through the ordinary contract-generation path, on the published
`.js` bytes rather than on a TypeScript source file written for a test.

## What each export is for

| export | marker kind | why |
| --- | --- | --- |
| `taggedForm` | `tagged-template` | the tag is invoked; no `calls` row exists |
| `getAccessorForm` | `get-accessor` | `box.value` resolves to a symbol with a get-accessor declaration |
| `setAccessorForm` | `set-accessor` | the same member in an assignment target position |
| `unknownMemberForm` | `property-access-unknown-accessor` | the receiver's member resolves to no symbol, so the producer cannot tell whether it is an accessor |
| `objectSpreadForm` | `property-access-unknown-accessor` | object spread reads every own enumerable property of a value whose shape is not statically known |
| `spreadForm` | `iteration-protocol` | drives `Symbol.iterator` and the iterator's `next` |
| `forOfForm` | `iteration-protocol`, then `coercion` | the same, plus `total + value`: the published `.js` annotates nothing, so that operand is not provably a non-object |
| `instanceofForm` | `instanceof` | reaches `Symbol.hasInstance` when the right operand defines it |
| `awaitThenableForm` | `await-then` | the awaited value is not provably a default-library `Promise` |
| `coercionForm` | `coercion` | a template span whose operand is not provably a non-object |
| `capturedTaggedForm` | `tagged-template`, captured | the form sits inside a callable the export hands back, so the row carries that callable and `captured` |
| `plainMemberForm`, `plainCallForm` | *none* | the negative controls |

The two controls are the point of the whole field. `plainMemberForm` reads a
member the checker resolves to a plain data property, which invokes nothing, and
`plainCallForm`'s body is one plain call the census already records.

`plainMemberForm`'s silence rests on *which* declaration resolves. `box.plain`
resolves to the constructor's own `this.plain = 2` — a runtime declaration in
`index.js` — and the producer only reads accessor declarations that are the
bytes that run. `index.d.ts`'s `readonly plain: number` would not have been
enough: a `.d.ts` `readonly` may perfectly well describe a getter, so an
accessor claim taken from one would be a claim about code nobody censused, and
the producer records `property-access-unknown-accessor` instead. That is why
this fixture is a published module with real `.js` bytes rather than a
declaration surface.

Both controls come back with `uncensusedInvokingForms` **present and empty**, which is the
producer's positive claim that every form it walked was a call, a construction,
or a node kind on its reviewed list of provably non-invoking kinds. An *absent*
list means a producer with no opinion; the handshake protocol is what separates
the two, never the list's emptiness.

## What is deliberately not here, and where it is instead

Four marker kinds cannot be expressed in a published ES module without either
changing what the fixture is or manufacturing a TypeScript diagnostic in it:

- `decorator` needs a decorator application, which is TypeScript or ES-decorator
  syntax rather than published JavaScript;
- `using-dispose` needs `using` / `await using`;
- `jsx-element` needs a `.tsx` source and a JSX namespace;
- `unclassified-invoking-form` — the load-bearing catch-all — has exactly one
  witness among the node kinds that can appear in a function body today, and it
  is `with`, which is a strict-mode error. Putting it in a checked-in fixture
  would introduce a TypeScript diagnostic here for no gain.

All four, and every kind above, are pinned per kind against the compiler in
`apps/solid-typefacts/internal/typefacts/tsgo/uncensused_invoking_forms_test.go`,
and the wire round trip through the real producer process is pinned in
`rust/crates/typefacts/tests/session_process.rs`.

## What the fixture's own expectations pin, and what pins the table above

`expected.json` and `expected-proposal.json` are the generated contract and
proposal. Neither carries the marker rows — `uncensusedInvokingForms` is a Type
Facts transcript field, not a contract field — so **they pin only that the
classifier runs over these shapes without changing the contract** the generator
produces for them. That is a real property (a classifier that refused a form the
generator depends on would move these files), but it is not where the marker
kinds are checked, and on its own it would leave the table above as prose
nothing verifies.

The table is pinned by
`TestUncensusedInvokingFormsClassifyThePublishedFixtureBytes` in
`apps/solid-typefacts/internal/typefacts/tsgo/uncensused_invoking_forms_test.go`,
which reads this directory's `index.js` off disk and asserts the exact kind
sequence per export. Editing `index.js` therefore moves a test, not only a
comment. That test builds a plain `allowJs` program rather than the generator's,
because what it pins is the classification of *these bytes*; the generator's own
consumption of the fixture is what the contract corpus pins.

**Both halves are deliberately weak today, and will not stay that way.** No
consumer reads `uncensusedInvokingForms` yet, so no contract, finding, or
receipt anywhere in the repository depends on any row here. The guarantee
becomes load-bearing at § 4.3 of
`docs/package-contract-v2/phase21/2026-09-03-implementation-census-plan.md` —
the implementation census that must enumerate every invoking form before it may
conclude that a behavioral call domain is closed. When that census exists, a
missing row in this fixture will be the difference between a refusal and an
unsound certification; until then it is the difference between two green tests.

The declaration signatures match the runtime API and are accepted by
TypeScript. Nothing here is a TypeScript diagnostic in disguise: the checker is
recording which syntactic positions can reach user code, which the type system
does not express.
