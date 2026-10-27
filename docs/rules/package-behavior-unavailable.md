# package-behavior-unavailable (SC9005)

An **obligation**, not a rule ([ADR 0440](../decisions.md#adr-0440)): an
imported package's required runtime behavior could not be established from
source. It is an **uncertifiable** result, never a proved violation, and an
internal one ([ADR 0328](../decisions.md#adr-0328)): the human formats and
ESLint do not show it and `--certify` ignores it, while `--format json` and
`--internal` keep it and the `uncertifiable` status it sets.

No option reaches it. It has no rule-options key and no ESLint rule, and
nothing disables it. A configuration that still names `package-behavior-unavailable`
or its former name `package-contract-incomplete` (and `package-contract-missing`,
`-export-missing`, `-callback-missing`) loads, and the key does nothing
(`RETIRED_RULES`).

Dependency source selection requires a declared runtime target and build under
the Vite and Solid plugin premises. Unsupported source selection, unavailable
files, unresolved callback behavior and source-edge refusals keep the exact
package obligation open, and so does selected source holding a call to a
value that is not callable, which no project `tsc` run checks (ADR 0368). Missing
module/type errors belong to TypeScript.

Provide analyzable runtime source and the correct host configuration where
supported. There is no contract, catalog or receipt workflow that can discharge
this obligation. Do not silence an unknown boundary by assuming package
behavior from a name. See [ADR 0312](../decisions.md#adr-0312).
