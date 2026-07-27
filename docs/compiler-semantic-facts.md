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

## Reference space

`EntityDemand.referenceSpace` produces `EntityFact.referenceSpace`:
`value`, `type`, `both`, or `neither`.

The retained reference index visits identifier nodes, resolves each with
`Checker.GetSymbolAtLocation`, excludes declaration/import-property names with
`ast.IsDeclarationNameOrImportPropertyName`, and classifies uses with
`ast.IsPartOfTypeNode`. Space is keyed by the local alias symbol rather than
its canonical target, so two imports of the same export may correctly have
different results.

## Canonical runtime identity

`EntityDemand.runtimeIdentity` produces `EntityFact.runtimeIdentity` when the
alias-resolved symbol has `SymbolFlagsValue` and a value declaration.

The adapter repeatedly follows `Checker.GetAliasedSymbol` through local
aliases and reexport chains. The equality key hashes the canonical value
declaration's normalized real path, byte range, and symbol name. This handles
named reexports, export-star chains, package subpaths, symlinked package
layouts, and symbols whose type and value declarations share a name.
`RuntimeSymbolID` is an equality key, not a `SymbolID` lookup handle.

The compact packed-table version is 3. The lifecycle protocol remains v3; its
schema digest changed with the coordinated Go and Rust model update.
