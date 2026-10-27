# Dependency source: every Solid package (ADR 0291)

`dependencySources: ["*"]` selects every Solid package the project loads at run time and every Solid package those load, each resolved from its importer (a Solid package's `dependencies`/`peerDependencies` name `solid-js`, `@solidjs/web`, `@solidjs/signals` or an `@solidjs/` package). Nested, `exports`-pattern and Vite extension-order packages give SC1001 at the call; packages with no Solid dependency give SC9012; `exports` array fallbacks are skipped with SC9012/SC9005. Stub `context-member-identity`'s (published rc.13).
