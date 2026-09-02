# Published runtime exports keep their exact runtime entity

This fixture pins the export-kind join used by published JavaScript packages
that ship adjacent declaration files. TypeScript resolves a sibling `.d.ts`
as the declaration surface for imports, but package-contract generation must
still decide runtime kind from the exact authenticated `.js` binding.

The runtime module exports one arrow-valued binding through its original name,
a renamed re-export, and a namespace re-export. The declaration surface repeats
the aliases. `plainValue` is the negative sibling: its runtime binding and
declaration type are both non-callable.

The expected contract therefore records `runtimeArrow`, `renamedArrow`, and
`runtimeNamespace` as the shapes proved by their exact runtime entities:

- the original and renamed arrow bindings are callable;
- the namespace exotic object and `plainValue` are plain values; and
- no kind is inferred from `const`, export-name spelling, or declaration text
  alone. An open or mixed runtime binding remains an artifact-case refusal.

The `./mixed` and `./unknown` subpaths pin that last boundary independently.
`mixedBinding` has exact callable and non-callable writes, while
`unknownBinding` reads an ambient host value with no authenticated runtime
definition. Neither subpath may publish a kind.

The fixture has no library dependency, so its `tsc --noEmit` oracle checks only
the real TypeScript standard library and does not rely on a permissive stub.
