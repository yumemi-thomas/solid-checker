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
   stable-v1 document and receipt per artifact case plus a
   `bundle-index.json`. Generate identical bytes under
   `rust/crates/solid-dialect/contracts/<id>/` and
   `pkg/contracts/bundled/<id>/`. Register package pins in the runtime lock.
6. Add or update the generated export modules in
   `solid-dialect/src/exports/`. Dialect tests must cross-check the vocabulary
   against accepted exact exports and must refuse incompatible same-spelling
   semantics.
7. Register one `solid_facts_backend::dialect::Dialect` value in
   `rust/crates/solid-facts-backend/src/dialect.rs` and add it to `ALL`. The
   bundle loader reads the manifest's indexes; do not add a parallel legacy
   decoder. Two things follow from `ALL` and need no decision:
   `default_dialect()` is the newest dialect it carries, and detection answers
   `Installed` for the new major instead of refusing it with `SC9013`. What
   still needs one is the `Version` variant — declare it **after** the existing
   ones, because `Ord` follows declaration order and that ordering is what
   `default_dialect()` reads.
8. Generate `packages/cli/lib/rules-<id>.json`. Its identity fields are
   `dialect`, compatibility `config`, and optional rule-name `namespace`.
   `SOLID_RULES_UPDATE=1 cargo test -p <id>-rules` writes the artifact; the
   adapter discovers it without a JavaScript registry.
9. Add end-to-end detection and dialect-pair fixtures proving at least one real
   semantic difference. Include exact-artifact refusal, local partial/open
   behavior, a consumer query, and a real-typings TypeScript oracle for each new
   package model.
10. Give the compiler adapter its own identity document and identity check.
    `scripts/check-compiler-facts-identity.mjs` verifies *2.0's* adapter
    against `docs/package-contract-v2/phase4/compiler-identity.json`, and the
    digest it recomputes has `solid-v2-compiler-source-manifest` inside the
    hashed string — so it cannot be generalized without changing the digest it
    exists to pin. A second compiler fork is a second document and a second
    check, not a loop over the first.
11. Update `rust/ARCHITECTURE.md`, rule documentation, runtime locks, and
    package-facing READMEs. If a payload may omit the dialect, expose and test a
    Cargo feature so its compiler and bundle are unreachable.

## Audit the shared code the new dialect passes through (ADR 0111)

Adding a dialect is also the moment the *other* dialect's assumptions become
visible. Every site in shared code that switches on a dialect id, a `Version`,
or a primitive **name** is one of two things, and each needs a decision rather
than a default:

- **dispatch on identity** — the code is reaching a specific dialect's compiled
  artifacts (`include_bytes!` corpora, a per-dialect compiler adapter). A match
  on the id is correct there. Keep it exhaustive or fail closed on an unknown
  id; never `_ => &[]`, which reports an empty answer as if it were a checked
  one.
- **role knowledge** — the code is asking what a runtime *does*, and a
  different dialect could answer differently. That belongs behind a `Dialect`
  question, and the question should be named for the property, not for the
  primitive that happens to have it today (`untracked_read_is_strict`, not
  `is_create_optimistic`).

Sites known to still need this decision are listed in
`docs/precision-backlog.md` (2026-09-17): `server_rules.rs` and
`project_server_rendering` hard-code `@solidjs/web` subpaths and export names,
and `owners.rs`, `static_api.rs`, `cleanup.rs` and `indexes.rs` have not been
reviewed. Read them before assuming shared code is dialect-neutral.

**Codes are shared deliberately; names are not.** Two catalogs may give one
concept the same `SCxxxx` so a suppression comment survives a migration
(`SC1001` is `strict-read-untracked` in every catalog that has the concept).
What must never happen is one code naming different rules in different
catalogs, because then a suppression means two things.
`scripts/second-dialect.test.mjs` asserts that, over the real catalogs and over
a synthetic second one.

## Verification

Use focused checks while assembling the dialect:

```sh
bun scripts/dialect-manifests.mjs validate
make contracts
make contract-conformance
bun run --cwd packages/cli test
bun packages/cli/node_modules/vitest/vitest.mjs run \
  --config packages/cli/vitest.config.mjs scripts/second-dialect.test.mjs
```

Beside it, `cargo +1.97 test -p solid-dialect --lib` carries
`a_dialect_that_states_nothing_claims_nothing`: a `Silent` dialect implementing
only the trait's 26 *required* methods, asked every defaulted question. Read it
before deciding which of the 36 defaults your dialect can leave alone — each
assertion is the conservative side, and the comment says what the other side
would have claimed. One default is deliberately not "nothing"
(`owner_requirement_role`), and the test says so rather than leaving it to be
discovered.

The last of the commands above is the check that a new dialect *arrives* where
the enumerators claim it does. It assembles a synthetic `solid-v3` in a throwaway tree and
demands that the manifest loader, the oracle and ownership gates' dialect sets,
the rule catalog every oracle case is checked against, the ESLint adapter's
discovery pattern, and coverage's fixture-stub major check all pick it up with
no edit. It covers the JavaScript half only: a missing `Version` variant, `ALL`
entry or vocabulary fails compilation, which is the intended way for those to
fail.

Finish with `make verify`. It checks Rust formatting/lints/tests, findings and
parity snapshots, TypeScript oracles, bundle byte/receipt identity, registry
pins, runtime locks, contract corpus and differential behavior, CLI/WASM
surfaces, and performance budgets.

## What remains centralized

A new dialect **does** state its own package-contract words, through
`Dialect::contract_callback_execution_at` (ADR 0111). That is a separate
question from `callback_execution_at`: the first is what a published contract
promises a consumer about scheduling relative to the exported call, the second
is attribution for the checker's own analysis, and they deliberately diverge for
some primitives. A dialect that states nothing yields "unknown", which is safe
but publishes no callback row.

New dialects do not add schema decoders, normalizers, receipt formats, package
name matchers, Makefile contract stanzas, probe registries, or ESLint manifest
maps. Shared code owns those deep seams and enumerates manifests. Rust still
names dialects at the composition root and in closed `Version` matches because
those are typed integration decisions whose omissions should fail compilation
or tests.

One `Version` match is worth knowing about before it surprises you:
`solid-reactive-ir`'s `effect_api.rs` refuses to guess where a dialect puts
`createEffect`'s apply slot. It is exhaustive on purpose, so adding a variant
breaks the build *there*, and until the new arm is written it fails closed —
`Uncertain` for the missing-effect proof and for owner registration. That is
the shape every such match should have.
