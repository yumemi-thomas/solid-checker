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
  For policies that accept only finite numbers, require `numbersAreFinite`
  whenever `mayBeNumber` is set; do not infer finiteness from a number literal's
  rendered type text.
- Whether an exported binding is a runtime *function*: demand `callability`
  **and** `constructability` at the export-specifier span, and treat
  `callable`, `untypedCallable`, or `constructable` as a function. Neither
  alone answers it: a
  class is `nonCallable`, because the type system reads a construct signature
  as not a call signature, so `callability` alone reports a class as a
  non-function. Do not substitute a syntactic search for a `class` keyword — a
  bundled class reached only through a value expression
  (`const C = (() => { class C {…}; …; return C; })()`, or a tuple element
  whose element type is a class) has no class expression to find. `unknown` on
  either fact, `mixed` on either fact, and absence are all fail-closed.
  Demand at the export-specifier span, never at a declaration name: a class
  *declaration name* (as opposed to the specifier that exports it) types as
  the class's instance type, which answers `nonCallable` + `nonConstructable`
  silently, with no `unknown` or `mixed` to flag the wrong span (see
  docs/compiler-semantic-facts.md and ADR 0020's "What it does not answer").
  **Handle `untypedCallable` before reading anything else off the row.** It is
  the signature-less `Function`-supertype family — `Function`,
  `CallableFunction`, `NewableFunction`, an alias or interface reaching them,
  and an intersection containing one — which declares `apply`/`call`/`bind` and
  no signature of its own. It is a *positive* answer, so
  `export declare const x: Function` is proof of a runtime function and may be
  published as one; what it withholds is every signature detail, so no arity,
  parameter type, or return type may be assumed and any check that needs one
  must fail closed. Note the pair's one deliberate disagreement: that same
  family answers `nonConstructable`, because `new x()` on it is a compile error,
  so `constructability` alone must not be read as "not a function" for a row
  whose callability is `untypedCallable`.
  **`nonCallable` + `nonConstructable` is proof that the declared type carries
  no call or construct signature *and* that the compiler refuses to call it** —
  which, since the Function-supertype family now answers `untypedCallable`, is
  as close to "the value is not a function" as a declared type can come. The
  residue is a type that *admits* a function value without being callable
  itself: `object`, `{}`, and `Record<string, unknown>` are not assignable to
  `Function`, the compiler refuses to call them, and they answer `nonCallable` +
  `nonConstructable`. `export declare const x: object` whose value happens to be
  a function is therefore published as a value, and that is the declared type's
  own claim, not a gap in the facts — no consumer of that export can call it
  either. A consumer that must be right about the runtime value regardless of
  what the author declared cannot use these facts for it; nothing in this
  vocabulary answers "what did the author actually assign".
  **Audit every existing `Callability` comparison before adopting this pin,
  because adding `untypedCallable` is source-compatible and every stale
  comparison below keeps compiling silently instead of failing to build.** A
  positive match on the exact variant misses it and must be widened: in
  solid-checker, `source_discovery.rs:1257`'s
  `== Some(typefacts::Callability::Callable)` and `contracts.rs`'s
  `export_kind_proof` positive arm,
  `(Some(Callability::Callable), _) | (_, Some(Constructability::Constructable))`,
  both need `Callability::UntypedCallable` added alongside `Callability::Callable`,
  and the `const CALLABILITIES: [Option<Callability>; 5]` test sweep in
  `contracts.rs` needs a sixth entry, `Some(Callability::UntypedCallable)`, or it
  silently stops covering the full enum the moment this value exists. A negative
  match on `NonCallable` needs no change: `runtime_semantics.rs`'s
  `!matches!(parameter.callability, Callability::NonCallable)` and
  `!matches!(callability, Some(Callability::NonCallable))` already read
  `untypedCallable` as "may be callable", which is the correct answer, without
  a single line moving. Do not paper over this by marking `Callability`
  `#[non_exhaustive]` in the producer — that would only turn a silent
  under-match into a silent `_` arm at the same call sites; the fix is to read
  every existing comparison at the consumer, not to make the compiler refuse to
  compile them.
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

- Enumerating an entrypoint's runtime-module closure by scanning source text for
  import specifiers and resolving them in the Node process: issue `modules` and
  record the inventory. The scan could disagree with the compiler in ways neither
  side reported; the inventory is the analyzing program's own file list, so the
  closure record becomes an attestation rather than a reconstruction. It names
  realpaths, so a pnpm store entry is one module rather than one per link, and it
  names the default library files the analysis opened.
- Applying a package contract to an import because the specifier's package root
  equals the contract's package name: demand `modules` with `packages` and
  require the import's `resolvedPath` to sit under the contract's package, or its
  `package.manifestPath` to be the contract's manifest. A `paths` alias that
  shadows an installed package answers `resolution: "nonRelative"` with a
  non-empty `pathsPattern` and an owning package that is not the one whose name
  it borrows; that combination must refuse the contract rather than apply it.
- Reasoning about a symlinked install from a path: read `resolvedPath` and
  `symlinkPath`. Do not derive one from the other, and do not treat a
  `.pnpm`-shaped path as evidence of anything.

The module graph does **not** close the declaration-sibling identity split. A
published `channel.d.ts` beside a `channel.js` is two unrelated modules to
TypeScript, which records nothing joining them, so nothing is reported. A
consumer seeing an import with a `.d.ts` extension, an empty `includedPath`, and
the runtime file present in the inventory as its own root must fail closed on it.
Pairing the two by matching file names is exactly the substitution the precision
contract forbids, and no fact here authorizes it.

`returnTypeText` remains presentation data. It must not be used to decide any
of the facts above. The new facts also do not authorize a callback-timing
contract: invocation timing is runtime behavior that TypeScript does not prove.
