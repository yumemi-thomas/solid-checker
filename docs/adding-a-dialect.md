# Adding a dialect

A dialect assembles a Solid vocabulary, JSX compiler adapter, rule catalog and
reviewed runtime model under one stable `solid-vN` id. Package behavior outside
that model requires exact source analysis; unknown behavior stays uncertifiable.

1. Add compiler and rules crates under `rust/dialects/<id>/` and register them
   in the workspace. Implement `CompilerFactsProvider` and `CatalogWording`.
2. Extend `solid_dialect::Version` and implement the vocabulary. Keep exact API,
   callback, ownership and release semantics behind the `Dialect` interface.
3. Add `dialect.json` naming the rule manifest and compiler identity documents.
   Validate with `bun scripts/dialect-manifests.mjs validate`.
4. Register the backend composition in `dialect::ALL`, including its compiler,
   catalog, runtime model and package-boundary finding projection. Check both
   detection and explicit selection, including unsupported-major refusals.
5. Generate the shipped rule manifest and teach adapters its stable identity.
6. Add focused positive, negative and unresolved fixtures using faithful
   published types; never duplicate TypeScript diagnostics.
7. Run compiler identity checks, armed process tests, coverage and ownership
   comparisons, then `make verify`. Review every semantic finding change.

The current compiler identity lives in
`rust/dialects/solid-v2/compiler-identity/compiler-identity.json`.
A dialect carries no per-package behavior bundle or receipt.
