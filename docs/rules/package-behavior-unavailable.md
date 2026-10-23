# package-behavior-unavailable (SC9005)

An imported package’s required runtime behavior could not be established from
source. This is an **uncertifiable** finding, never a proved violation.
The former name `package-contract-incomplete` is still accepted as an alias in
`rule-options.json` and in ESLint configurations.

Dependency source selection requires a declared runtime target and build under
the Vite and Solid plugin premises. Unsupported source selection, unavailable
files, unresolved callback behavior and source-edge refusals keep the exact
package obligation open, and so does selected source holding a call
TypeScript rejects, which no project `tsc` run checks (ADR 0368). Missing
module/type errors belong to TypeScript.

Provide analyzable runtime source and the correct host configuration where
supported. There is no contract, catalog or receipt workflow that can discharge
this obligation. Do not silence an unknown boundary by assuming package
behavior from a name. See [ADR 0312](../decisions.md#adr-0312).
