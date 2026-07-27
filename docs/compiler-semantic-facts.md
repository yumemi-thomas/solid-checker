# Compiler semantic facts

The package-contract generator can demand four facts without reconstructing
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
are cached by signature within one analysis generation and discarded on
update. Retained per-file contributions track declaration and parameter source
dependencies, so an edit rematerializes only facts that could otherwise carry
stale locations.

The lifecycle protocol/schema version is 4. The compact packed-table version is
3 and the packed-delta version is 2. Go and Rust pin the same v4 schema digest,
so mismatched producer/client versions fail during the startup handshake.
