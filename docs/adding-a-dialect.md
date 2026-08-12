# Adding a dialect

A dialect is one Solid language version assembled from four contributions: a
vocabulary, a JSX compiler adapter, a rule catalog, and bundled package
contracts. Use one stable id of the form `solid-vN` everywhere. Package-specific
targets append a slash and package slug, for example `solid-v3/solid-js`.

The checked-in `rust/dialects/<id>/dialect.json` file is the assembly manifest.
Contract generation, drift checks, runtime conformance, and the shipped ESLint
catalog discovery all key off these manifests or the artifacts they name. Run
`node scripts/dialect-manifests.mjs validate` early: a missing or duplicate
artifact is then a hard failure instead of a dialect that silently does not
ship.

## Checklist

1. Add the compiler and rules crates under `rust/dialects/<id>/`, register both
   in `rust/Cargo.toml`, and pin any upstream compiler dependency there. The
   compiler crate implements `CompilerFactsProvider`; the rules crate implements
   `CatalogWording`, exposes `solve_measured`, and owns all external prose.
2. Extend `solid_dialect::Version`, implement the vocabulary in
   `rust/crates/solid-dialect/src/`, and export it from the crate root. Keep API
   spellings, callback/owner semantics, module ownership, and boundary behavior
   in that implementation rather than switching on names in shared analysis.
3. Add `rust/dialects/<id>/dialect.json`. For each modeled package declare its
   package path environment variable, stable generator target, review contract,
   generated export index, and bundled runtime contract. Set `probeRuntime` for
   contracts checked against an installed release. If a bundled artifact is
   composed from checked-in inputs, declare `composeScript` and `composeInputs`.
4. Add the package extraction semantics for each new generator target in
   `solid-contract-gen`, then run `make contracts`. Register the generated
   export module in `solid-dialect/src/exports/mod.rs` and consume it from the
   vocabulary implementation. Review contracts live below
   `solid-dialect/contracts/<id>/`; runtime contracts live below
   `pkg/contracts/bundled/<id>/`.
5. Register one `solid_facts_backend::dialect::Dialect` value in
   `rust/crates/solid-facts-backend/src/dialect.rs`, including its compiler,
   catalog, documentation, and bundled-package functions. Add it to `ALL`, map
   the corresponding `Version`, and decide whether project-version detection
   or the default changes.
6. Decode the bundled contract set in `diagnostics.rs`. Its evidence URI and
   `Dialect::bundled_contract_label` must use `<id>/<package-slug>.json`, matching
   the artifact path below the contracts root.
7. Generate the ESLint rule artifact as
   `packages/cli/lib/rules-<id>.json`. Its identity fields are `dialect` (the
   stable id), `config` (the backward-compatible flat-config key), and
   `namespace` (the rule-name prefix, or the empty string). Use
   `SOLID_RULES_UPDATE=1 cargo test -p <id>-rules` to write it. The adapter
   enumerates matching files; no JavaScript registry edit is needed.
8. Add an end-to-end detection fixture and a dialect-pair coverage fixture that
   proves at least one semantic difference. Extend only genuinely closed Rust
   matches and public request documentation, such as the wasm `dialect` field;
   the coverage runner discovers ordinary fixture directories.
9. Update `rust/ARCHITECTURE.md`, rule documentation, and any package-facing
   README. If the dialect can be omitted from a payload-specific build, expose
   and verify the corresponding Cargo feature rather than leaving its compiler
   reachable.

## Verification

Run these narrow checks while assembling the dialect:

```sh
node scripts/dialect-manifests.mjs validate
make contracts-check
make contract-conformance
npm test --prefix packages/cli
```

Finish with `make verify`. The full workflow checks Rust formatting and lint,
all workspace tests, findings/parity snapshots, performance budgets, the CLI
adapter, manifest integrity, runtime contract conformance, and composed-contract
drift.

## What the manifest removes

A new dialect does not require a Makefile contract stanza, a runtime-probe
entry, an ESLint manifest map entry, or dialect-specific JavaScript branching.
Those surfaces enumerate the assembly data. Rust still names dialects at the
composition root and in closed `Version` matches on purpose: those are typed
integration decisions whose omissions should fail compilation or tests.
