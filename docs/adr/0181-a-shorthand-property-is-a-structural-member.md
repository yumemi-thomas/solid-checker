# ADR 0181: A shorthand property is a structural member

- Status: accepted and implemented (2026-10-05). Fifth lever of the owner's
  package-misuse goal of 2026-10-04.
- Owners:
  - the Type Facts producer's literal census (`return_structures.go`) and its
    reference resolution (`referenceSymbolLocked`);
  - the Oxc literal facts (`ReturnStructureFact::complete_literal`);
  - the generator's member resolution (`literal_structural_returns`);
  - the certification census's key check (`structural_returns::shape`).
- Relation: amends ADR 0172, whose initial grammar refused shorthand
  properties. No wire change. The producer's source identity changes, so
  `bin/solid-typefacts` is rebuilt.

## Context

Four runtime-detected strict reads in the misuse ledger were blocked by "live
return has no exhaustive literal structure". Each returns an object written
with shorthand properties:

- `createOrientation`: `return { angle, type }`;
- `createNotification`: `return { show, close, notification, supported }`;
- `createEventStack`: `return { …, value: stack, setValue, remove }`;
- `createFullscreen`: `return { enter: …, exit: …, isActive }`.

Shorthand `{ value }` means `{ value: value }`. Hook-style returns use it
constantly, so the refusal reached far beyond these four. ADR 0172 refused it
with numeric and computed keys, as a key/value form needing its own census.
The obstacle is real but narrow: TypeScript answers a symbol query at a
shorthand name with the object literal's *property*, not the value binding it
reads.

## Decision

1. **The producer accepts a shorthand property.** Its key is the identifier,
   and its value leaf is that same identifier. Leaf evidence resolves the
   identifier through `referenceSymbolLocked`, which answers a shorthand name
   with `GetShorthandAssignmentValueSymbol`, as the parameter-use census
   already does. A shorthand default (`{ value = 1 }`, a destructuring target
   only) and string, numeric or computed keys on a shorthand stay refused.
2. **The certification census accepts a key equal to its value span.** Every
   other key/value overlap still refuses.
3. **The generator proposes shorthand members.** The Oxc literal check
   accepts a shorthand whose value is an identifier. A member that TypeScript
   does not resolve to a parameter is resolved through the binder's own
   reference table (`AstFacts::reference_declaration`).

## Consequences

- The parameter-use census is unchanged. A parameter stored in a shorthand
  property is still an unknown escape there.
- The structural census proves every member exactly as before; only the
  key/value spelling changed.

## Evidence

- Producer tests: `TestReturnStructuresAreExhaustiveFreshLiteralCensuses`
  now expects `({value})` and `({value, x: 1})` to be complete objects. The
  shorthand member carries the caller's parameter identity, not the
  property's. The whole producer suite passes.
- Census test
  `structural_object_returns_accept_a_shorthand_property_at_one_span`: a
  shorthand is accepted, and an overlapping key that is not its value is
  refused.
- Browser tier regenerated with `--carry`:
  - the misuse ledger goes from 70 to 73 static violations of 123:
    `createEventStack` and `createFullscreen`, both runtime-detected
    (`STRICT_READ_UNTRACKED`), and `createDropzone`;
  - the ledger records `createDropzone` as `harness-error`: the harness's
    dev server answered 500 for that case, for both twins. Its finding is a
    top-level read of `files`, a `createSignal([])` accessor returned by
    shorthand, which is the confirmed mechanism of the other two. Its correct
    twin, the same read in JSX, is not flagged;
  - no correct twin is flagged, and no checkpoint row changes status;
  - per environment, ten exports gain a returned-accessor claim
    (`createFormControl` twice, `createBroadcastChannel`, `createVideo`,
    `createListState`, `createMultiSelectListState`, `createDropzone`,
    `createFilePicker`, `createForm`, `createFullscreen`,
    `createEventStack`), and none loses one.

  `createOrientation` and `createNotification` remain behind other walls.
