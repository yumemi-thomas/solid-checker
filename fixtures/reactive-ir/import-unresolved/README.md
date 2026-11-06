# import-unresolved

**Claim (ADR 0827).** A value imported through an import the compiler does not resolve (a missing module, an export its target lacks, an import silenced by `@ts-ignore`) is unknown: each runtime use of it, a call in JSX or the body, an argument to `createEffect`, a component tag, a write through it, a namespace member, is SC9012 uncertifiable `import-unresolved`, never a clean. The same uses of a resolved import (`./local`) are not this row, nor is an unresolved import used only in a type position.

Before ADR 0827 each of these certified; in a project that also analyzes `@solidjs/router` as source, some were U only because the unknown symbol was named `createOwner` (ADR 0826).
