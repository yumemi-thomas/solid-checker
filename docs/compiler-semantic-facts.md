# Compiler semantic facts

The package-contract generator can demand five facts without reconstructing
TypeScript semantics from text.

## Callability

`EntityDemand.callability` produces `EntityFact.callability`:
`callable`, `nonCallable`, `mixed`, or `unknown`.

The TypeScript-Go adapter obtains the expression type with
`Checker.GetTypeAtLocation`, distributes real union constituents with
`Type.Distributed`, and asks `Checker.GetSignaturesOfType` for
`SignatureKindCall`. Construct signatures do not count. `any`, `unknown`,
`never`, compiler error types, and missing types report `unknown`. No
`TypeToString` result participates in this decision.

## Runtime value domain

`EntityDemand.runtimeValueDomain` produces
`EntityFact.runtimeValueDomain: *RuntimeValueDomain` in Go and
`Option<RuntimeValueDomain>` in Rust. The fact has four booleans:

- `mayBeCallable`: at least one possible value has a TypeScript call signature.
- `mayBeUndefined`: at least one possible value is `undefined`.
- `mayBeOther`: at least one possible value is neither callable nor undefined.
- `unknown`: TypeScript-Go exposed a dynamic, unresolved, circular, or recovery
  domain that cannot be exhaustively classified.

The checker adapter recursively ORs the classifications of actual union
constituents. Non-union structured types are callable when
`Checker.GetSignaturesOfType(type, SignatureKindCall)` returns a signature;
this covers functions, overloads, callable interfaces, aliases, and callable
intersections. `TypeFlagsUndefined` is undefined, and a remaining intersection
that the checker proves assignable to its canonical undefined type is also
undefined. Every other known, inhabited type is `other`.

For an instantiable type, the adapter follows TypeScript-Go's resolved generic
constraint before classifying it. Thus `T extends (() => void) | undefined`
has the same closed domain as its constraint, while an unconstrained `T` is
unknown. `never` is the known empty domain: all four fields are false. `any`,
`unknown`, unconstrained or circular generics, missing types, and types carrying
`TypeFlagsIncludesError` conservatively set all three `mayBe` fields and
`unknown` to true. No rendered type text participates.

For Solid 2 cleanup validation, a present fact is exclusively in the accepted
runtime domain exactly when:

```text
mayBeOther == false && unknown == false
```

This accepts callable values, `undefined`, their unions, and the vacuous
`never` domain. Consumers should not infer that a missing fact is valid; absence
means it was not demanded.

## Resolved-call validity

`EntityDemand.resolvedCall` always produces a resolved-call fact. Its
`validity` is:

- `valid`: normal overload resolution selected an applicable signature.
- `recovery`: TypeScript returned its unknown signature or an overload-failure
  candidate while recovering from a failed call.
- `unresolved`: the demanded node does not resolve to a call expression or no
  signature is available.

The adapter uses `getResolvedSignature`, its compiler candidate list,
`SignatureFlagsIsSignatureCandidateForOverloadFailure`, and the compiler's
call-resolution diagnostic codes at that call. Consumers do not inspect
diagnostics and must only treat `valid` as positive evidence.

## Selected declaration identity

For each demanded call or `new` expression, `resolvedCall.kind` is `call` or
`construct`. A valid, non-composite signature also carries
`resolvedCall.declaration`:

- `symbol` is the canonical compiler symbol for the selected signature
  declaration.
- `location` and `kind` identify the exact overload declaration returned by
  `Signature.Declaration`.
- `owners` is the outermost-to-innermost chain of compiler declaration
  containers, with a symbol, declaration kind, and location for each.
- `qualifiedName` joins the compiler symbol names from that chain for display,
  such as `Storage.getItem`; it is not the equality key.
- `originModule`, `sourceFile`, and `standardLibrary` report compiler-derived
  provenance when it exists.

The adapter obtains the selected signature with `getResolvedSignature`, follows
aliases with `Checker.GetAliasedSymbol`, and derives every owner from the
declaration AST and its symbols. Identity therefore comes from the selected
signature symbol plus its containing declaration symbols and locations, not
from a member-name lookup or source parsing. Two declarations named `push` on
`Array` and a structural user type have different symbol/owner identities.

An incremental compiler can retain a signature object whose declaration node
belongs to the preceding source generation. Before emitting locations, the
adapter maps that declaration through the current target symbol's declarations.
If no current declaration can be established, it omits the selected declaration
instead of publishing a stale location.

## Exhaustive target candidate sets

A valid composite call — one whose callee type is a union — carries no single
selected declaration. Since lifecycle v9, it may instead carry
`resolvedCall.targets`, a `CallTargetSet` with an explicit `exhaustive` proof
bit and deterministically ordered, deduplicated candidate declarations. The
set is emitted only when every union constituent is one closed concrete
callable and every one of its call (or construct) signatures names one exact
implementation declaration (`FunctionDeclaration`, `MethodDeclaration`,
`ArrowFunction`, `FunctionExpression`, `Constructor`) with a canonical symbol.
`any`, `unknown`, nullable, generic, intersection, error, type-level, and
declaration-less constituents void the whole set; an incomplete candidate set
is never published. The proof covers the callee's apparent type — the same
evidence class as the single selected declaration — so consumers must compare
each candidate's analyzed behavior before certifying the dispatch, and must
keep divergent or unresolved candidates fail-closed. Argument mappings remain
`compositeSignature` for composite callees because per-candidate mappings may
differ.

## Argument-to-parameter mapping

Every supplied argument has an `ArgumentMapping`:

- `resolved` includes the formal parameter index, current declaration location
  when available, parameter symbol identity, rest/optional flags, callability,
  and a type descriptor after generic substitution.
- `unresolved` includes one of `callUnresolved`, `recoverySignature`,
  `compositeSignature`, `spreadArgument`, or `parameterUnavailable`.

The formal parameter comes from `Signature.Parameters`; rest and minimum
argument information come from `Signature.HasRestParameter` and
`Signature.MinArgumentCount`. The instantiated parameter type comes from the
checker operation corresponding to TypeScript's `getTypeAtPosition`, so generic
calls report the selected substitution rather than the declaration's type
parameter. Callability is then calculated from actual call signatures using the
same rules as demanded expression callability.

Recovery and unresolved calls never expose a parameter as resolved. Spread
arguments remain explicit `spreadArgument` mappings because one spread can
cover zero or several formal positions. A synthesized composite signature for
a union callable reports `compositeSignature`: TypeScript proves the call but
does not expose one underlying declaration/parameter identity that the producer
can safely choose. Intersection overloads are mapped when resolution selects a
real constituent declaration.

`TypeDescriptor.text` and `returnTypeText` are display metadata. They do not
participate in declaration identity, mapping, validity, or callability.

These facts say nothing about callback timing or retention. TypeScript's type
system cannot prove whether a callback runs inline, later, or at all.

## Reference space

`EntityDemand.referenceSpace` produces `EntityFact.referenceSpace`:
`value`, `type`, `both`, or `neither`.

The retained reference index visits identifier nodes, resolves each with
`Checker.GetSymbolAtLocation`, excludes declaration/import-property names with
`ast.IsDeclarationNameOrImportPropertyName`, walks each identifier through its
enclosing `QualifiedName` chain, and classifies the resulting compiler node
with `ast.IsPartOfTypeNode`. Walking the AST chain makes the surrounding
`TypeReference` or `TypeQuery` authoritative for leftmost namespace
identifiers. Space is keyed by the local alias symbol rather than its canonical
target, so two imports of the same export may correctly have different
results.

## Explicit symbol resolution

`EntityDemand.symbol` produces exactly one of three outcomes at the demanded
location:

- `EntityFact.symbol` is non-empty when TypeScript-Go resolves a symbol;
- `EntityFact.symbolUnresolved` is true when the source node exists and
  `Checker.GetSymbolAtLocation` explicitly returns no symbol;
- both fields are empty when the producer could not inspect the source node.

The last case is unavailable evidence, not proof that a binding is missing.
Consumers may diagnose an undefined name only from `symbolUnresolved`; they
must not reinterpret an empty row as a negative answer. A demand for a narrow
root inside a JSX member tag stays narrow, while a demand spanning the complete
tag resolves the selected member.

## Canonical runtime identity

`EntityDemand.runtimeIdentity` produces `EntityFact.runtimeIdentity` when the
alias-resolved symbol has `SymbolFlagsValue` and a value declaration.

The adapter repeatedly follows `Checker.GetAliasedSymbol` through local
aliases and reexport chains. The equality key hashes the canonical value
declaration's normalized real path, byte range, and symbol name. This handles
named reexports, export-star chains, package subpaths, symlinked package
layouts, and symbols whose type and value declarations share a name.
`RuntimeSymbolID` is an equality key, not a `SymbolID` lookup handle.

Resolved-call work is demand-driven: the producer resolves and describes only
requested call locations. Selected declarations and instantiated parameters
are cached by signature within one analysis generation, while compiler-identical
instantiated and return types share one descriptor rendering. All checker-owned
caches are discarded on update. Declaration, owner, and parameter identities
are embedded facts rather than standalone symbol-closure lookup handles, so
they do not duplicate symbol rows. Retained per-file contributions track
declaration and parameter source dependencies, so an edit rematerializes only
facts that could otherwise carry stale locations.

The active lifecycle schema is v8 and the active Wire table model is v5. The
packed transition framing remains version 1. Frozen lifecycle v5/v6 adapters
continue to emit Wire table v3, and frozen v7 emits Wire table v4. Go and Rust
pin the same per-schema digest, so mismatched producer/client versions fail
during the startup handshake.
