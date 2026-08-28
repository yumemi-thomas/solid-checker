# Adding a dialect

A dialect is one Solid language version assembled from a vocabulary, JSX
compiler adapter, rule catalog, and receipt-issued package-contract bundle. Use
one stable id of the form `solid-vN` everywhere. Package contracts remain exact
published artifact cases; a dialect id never substitutes for package identity.

The checked-in `rust/dialects/<id>/dialect.json` is the assembly manifest.
Contract generation, bundle drift checks, runtime conformance, and shipped
ESLint catalog discovery enumerate it. Validate the manifest early:

```sh
bun scripts/dialect-manifests.mjs validate
```

## Checklist

1. Add compiler and rules crates under `rust/dialects/<id>/` and register them
   in `rust/Cargo.toml`. The compiler implements `CompilerFactsProvider`; the
   rules crate implements `CatalogWording`, exposes `solve_measured`, and owns
   all external wording.
2. Extend `solid_dialect::Version`, implement vocabulary under
   `rust/crates/solid-dialect/src/`, and export it from the crate root. Keep API
   spellings, callback/owner semantics, module ownership, and boundaries in the
   dialect rather than switching on names in shared analysis.
3. Add `rust/dialects/<id>/dialect.json` with the rule manifest,
   `bundleIndex`, `reviewBundleIndex`, and every modeled package. Mark packages
   whose checked authority includes runtime observations with `probeRuntime`;
   list finite probe modes when they differ.
4. Build a checked semantic authority for every package/artifact case. Acquire
   exact package name, version, registry integrity, runtime and declaration
   files, conditional-export traces, exact export identities, and transitive
   closure. Encode behavior in the normalized model and identify local open
   domains; do not copy an existing dialect contract by API name.
5. Run the Rust proof checker over the checked corpus to emit one deterministic
   temporary-v2 document and receipt per artifact case plus a
   `bundle-index.json`. Generate identical bytes under
   `rust/crates/solid-dialect/contracts/<id>/` and
   `pkg/contracts/bundled/<id>/`. Register package pins in the runtime lock.
6. Add or update the generated export modules in
   `solid-dialect/src/exports/`. Dialect tests must cross-check the vocabulary
   against accepted exact exports and must refuse incompatible same-spelling
   semantics.
7. Register one `solid_facts_backend::dialect::Dialect` value in
   `rust/crates/solid-facts-backend/src/dialect.rs`, add it to `ALL`, map its
   `Version`, and make an explicit default/detection decision. The bundle
   loader reads the manifest's indexes; do not add a parallel legacy decoder.
8. Generate `packages/cli/lib/rules-<id>.json`. Its identity fields are
   `dialect`, compatibility `config`, and optional rule-name `namespace`.
   `SOLID_RULES_UPDATE=1 cargo test -p <id>-rules` writes the artifact; the
   adapter discovers it without a JavaScript registry.
9. Add end-to-end detection and dialect-pair fixtures proving at least one real
   semantic difference. Include exact-artifact refusal, local partial/open
   behavior, a consumer query, and a real-typings TypeScript oracle for each new
   package model.
10. Update `rust/ARCHITECTURE.md`, rule documentation, runtime locks, and
    package-facing READMEs. If a payload may omit the dialect, expose and test a
    Cargo feature so its compiler and bundle are unreachable.

## Verification

Use focused checks while assembling the dialect:

```sh
bun scripts/dialect-manifests.mjs validate
make contracts
make contract-conformance
bun run --cwd packages/cli test
```

Finish with `make verify`. It checks Rust formatting/lints/tests, findings and
parity snapshots, TypeScript oracles, bundle byte/receipt identity, registry
pins, runtime locks, contract corpus and differential behavior, CLI/WASM
surfaces, and performance budgets.

## What remains centralized

New dialects do not add schema decoders, normalizers, receipt formats, package
name matchers, Makefile contract stanzas, probe registries, or ESLint manifest
maps. Shared code owns those deep seams and enumerates manifests. Rust still
names dialects at the composition root and in closed `Version` matches because
those are typed integration decisions whose omissions should fail compilation
or tests.
