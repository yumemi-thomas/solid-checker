# solid-checker migration

The package-contract generator can remove these heuristics after consuming the
new facts:

- Regex and rendered-type checks for function-like syntax: demand
  `callability`.
- `TypeDescriptor.text` and `typeToString` interpretation for callability:
  use the callability enum.
- Synthetic probe calls and diagnostic filtering: demand `resolvedCall` and
  accept only `valid`.
- Source-text searches that decide whether an import is type-only or runtime:
  demand `referenceSpace`.
- Regex parsing of export declarations and module specifiers to join aliases:
  compare non-empty `runtimeIdentity` values.

`returnTypeText` remains presentation data. It must not be used to decide any
of the facts above.
