# solid-checker migration

The package-contract generator can remove these heuristics after consuming the
new facts:

- Regex and rendered-type checks for function-like syntax: demand
  `callability`.
- `TypeDescriptor.text` and `typeToString` interpretation for callability:
  use the callability enum.
- Cleanup return validation that treats both `(() => void) | undefined` and
  `(() => void) | number` as mixed callability: demand `runtimeValueDomain`,
  require a present fact, and accept exactly when
  `!domain.may_be_other && !domain.unknown`.
- Cleanup return validation for a returned call: demand `callResultDomain` at
  the exact call span and classify that result domain. Do not use the generic
  `runtimeValueDomain` demand at a call span for this purpose; it describes the
  callee. Treat an absent or `unknown` call-result domain as fail-closed.
- Static JSX attribute strings for `no-innerhtml`'s `allowStatic` and
  `jsx-no-script-url`: demand `constantValue` at the exact attribute-value
  expression span and accept only a present value with `kind: "string"`. Do
  not recover static strings from literal-type text or `typeDescriptor.text`;
  absence means not proven static.
- Array/tuple shape questions, such as whether a JSX event-handler value is an
  array or whether a `.map` receiver is a real array: demand `arrayShape` at the
  exact expression span. Do not test `typeDescriptor.text` for `[`, `Array<`,
  `ReadonlyArray<`, `readonly `, or a trailing `[]`; an aliased tuple renders as
  its alias and defeats all of them, and a trailing `[]` cannot distinguish an
  array of functions from a function returning an array. `arrayShape` needs no
  companion `callability` demand to make that distinction. Only `notArray`
  proves the negative; `mixed`, `unknown`, and absence are fail-closed.
- Whether a value satisfies an interface with *numbered* members, such as a
  `[handler, data]` pair: demand `tupleShape` and require the slot to exist
  (`fixedLength`, plus `hasRest` for the tail) and `elementZero` to be
  `callable`. `arrayShape` cannot answer this — it reports `array` for a plain
  array too, and a plain array has no `0`/`1` property, so it is a type error
  rather than your finding. Absence means not proven a tuple.
- Exact runtime arity for a spread tuple: demand `tupleShape` and require
  `exactLength`. Do not use `fixedLength`; it counts optional slots and a union
  meet keeps only common slots, neither of which proves how many values will be
  spread at runtime. Rest, variadic, optional, and unequal-union tuples leave
  `exactLength` absent.
- Whether a value is one of a set of well-known runtime types (`Date`, `Map`,
  `Set`, a typed array): demand `libraryTypes` and match the returned names
  against your own list. Do not split `typeDescriptor.text` on `|`/`&` and match
  heads: an alias renders as its own name and matches nothing, `Array<Date>` and
  `Date[]` are the same value but read differently, and a user-defined type can
  share a global's name. Absence means nothing at the top level was a library
  type.
- Primitive-kind questions about declared values: demand
  `primitiveValueDomain` at the exact expression span. Use its closed category
  set instead of `TypeDescriptor.text`; aliases and constrained generics keep
  their compiler meaning, while `any`, `unknown`, recovery types, and an absent
  fact remain fail-closed. Serialization policy stays in the consumer — this
  fact reports language value categories, not “JSON safe”.
- Synthetic probe calls and diagnostic filtering: demand `resolvedCall` and
  accept only `valid`.
- Source-text searches that decide whether an import is type-only or runtime:
  demand `referenceSpace`.
- Regex parsing of export declarations and module specifiers to join aliases:
  compare non-empty `runtimeIdentity` values.
- Declaration-name plus declaration-file checks such as `"getItem"` in
  `lib.dom.d.ts`: compare the selected declaration symbol and owner chain from
  `resolvedCall.declaration`. `qualifiedName` is useful for logs, not equality.
- Overload guessing and positional argument heuristics: consume each
  `resolvedCall.arguments` entry and use its selected parameter symbol,
  declaration, instantiated type, rest/optional flags, and callability.
- Synthetic or best-effort mapping for recovery, union-composite, and spread
  calls: honor the explicit unresolved mapping reason instead.

`returnTypeText` remains presentation data. It must not be used to decide any
of the facts above. The new facts also do not authorize a callback-timing
contract: invocation timing is runtime behavior that TypeScript does not prove.
