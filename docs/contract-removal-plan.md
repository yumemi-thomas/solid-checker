# Plan: giving up package contracts

Status: approved direction (owner, 2026-10-14). Not started.

Dependency behaviour comes from analyzing installed package source (ADRs 0284,
0290, 0291, 0295–0300). Package contracts are removed: first their
consumption, then, one release later, the whole emission and certification
pipeline.

## Decisions

1. **No declared host, or no Solid Vite plugin.** Infer the host where it can
   be proven (ADR 0270). Otherwise every package call is uncertifiable.
2. **WASM.** A host protocol: the JavaScript host supplies package files,
   their Type Facts and resolution rows, and WASM runs the same source
   analysis.
3. **Non-Solid packages.** Analyzed from source when a selected package
   re-exports or forwards their values. Any other package stays unknown
   behaviour, which is uncertifiable.
4. **Pipeline.** Contract consumption is switched off first. The code stays
   for one release as a rollback, then everything contract-only is deleted.
5. **Rollback.** The switch-off is one commit, rolled back with `git revert`.
   There is no hidden flag, so no contract gate or fixture is kept alive for
   that release.
6. **Read-only projects.** The combined-program tsconfig is written to a
   scratch directory with the same anchoring, so these projects still get
   source analysis.
7. **Host-free runs, two passes.** The host is inferred from project code
   first, then package source is selected and analyzed under it. If the build
   (development or production) is not proven, or scopes mix browser and server
   execution, package calls stay uncertifiable.

Premises a clean result rests on, stated in reports: Vite with the Solid
plugin, standard runtime configuration, package integrity (ADR 0300), and the
Window realm where declared (ADR 0295).

## Phases

Each phase lands with fixtures, an adversarial review round until it comes
back clean, a per-site corpus sweep, and `make verify`.

### 0. Measure

A corpus sweep in source mode with every contract disabled, compared per site
with contract mode. The difference is the work list for phase 1.

### 1. Close the source gaps

These are the six packages the corpus still keeps on contracts:

| Gap | Packages | Work |
| --- | --- | --- |
| Namespace imports | `@kobalte/core`, `@solid-primitives/controlled-signal` | redirect `ns.member` exactly; refuse escaping uses |
| Non-Solid re-export | `@rigelbuild/solid-virtual` → `@tanstack/virtual-core` | decision 3 |
| `browser` field map | `@rigelbuild/solid-markdown` | apply the map for a browser host, as Vite does |
| `?raw` / JSON modules | `@kobalte/solidbase`, `@solidjs/start` | model them as inert data modules |

Also in this phase:
- the contract-mode `export *` gap stops mattering once contracts are gone.
  Until then it is fixed (ADR 0301, in progress);
- optionally, kui's router getter through package-object escape analysis
  (parked in docs/precision-backlog.md).

### 2. Host-free runs

Host inference supplies a browser host where it is proven, without contract
views. An unprovable host means package calls are uncertifiable, and SC9005 is
reworded to "package behaviour unknown" or replaced.

### 3. WASM host protocol

Extend the WASM request with selection rows, package files, Type Facts for the
combined program, and fold inputs. Selection, redirects, folding and analysis
are shared with the native path.

### 4. Switch off consumption

- No catalog, authored tier or bundled contract is bound.
- `--no-dependency-source` is removed.
- Snapshots, gates and docs move to source-only.
- The pipeline code stays one release as the rollback.

### 5. Delete

Everything that exists only for contracts goes:
- consumption, catalogs, admission, trust and receipts;
- the authored tier, emission and certification;
- probes and proof sidecars;
- the contract corpus and registry pins;
- the schema, CLI contract commands, their fixtures, gates and docs.

The exact inventory is in
`rust/target/research/contract-deletion-inventory.md`, checked against
`8f70a211e`. It covers about 250k Rust and Go lines, 110k JavaScript lines and
3,000 data files. Some IR types named `Contract*` carry ordinary analysis
summaries, so they are renamed rather than deleted.
Shared parts stay: the dialect vocabulary, Solid's runtime model, Type Facts
used by ordinary analysis, host constants and host inference.

## Removal order

Each step lands green on its own. The inventory's section 5 has the details.

| Step | Content | Size |
| --- | --- | --- |
| P0 | Measure: per-site sweep with contracts disabled; a dual-mode comparison script | small |
| P1 | Source gaps: namespace redirects, non-Solid packages, `browser` map, `?raw`/JSON modules; seed a source-selection corpus gate | large |
| P2 | Neutral IR vocabulary: move the `Contract*` summary types (no behaviour change) | medium |
| P3 | `PackageBoundary`: imports that cannot be analyzed, each with a reason; host passed separately (no finding moves) | medium |
| P4 | Host-free runs (decision 7) | large |
| P5 | WASM host protocol | large |
| P6 | Move callers and fixtures off contract mode; scratch tsconfig for read-only projects | large |
| P7 | Switch consumption off (one commit, the revert point) | medium |
| — | one release | — |
| D1–D8 | Delete consumption (backend, IR), emission, certification and probes, the dialect's negative authority, the Type Facts certification protocol, historical ledgers and benchmarks, schemas and docs | large in total |

These constraints are fixed:
- P2–P6 come before P7.
- P1's seed recycling comes before D3.
- D6 comes after D4.

## Risks

- Every new source path needs review rounds; so far each round has found
  contrived soundness holes, all of them fixed.
- The corpus is small (49 projects). Use the seeded-defect harness
  (`solid-checker-rc13-corpus/experiments/recall`) and bug-fix mining to
  check that recall does not drop at each phase.
- Host-free users lose package behaviour until inference covers them.
