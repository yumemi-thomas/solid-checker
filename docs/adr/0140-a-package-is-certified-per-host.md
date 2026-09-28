# ADR 0140: A package is certified per host

- Status: accepted and implemented (2026-09-28); written with the implementation
- Date: 2026-09-28
- Owners: the host vocabulary (`solid-dialect/src/lib.rs`,
  `HostTargetCondition`), consumer case selection (`contract_interface.rs`
  `admissible_cases`, shared by project catalogs and compiled-in bundles), the
  artifact-case census (`packages/cli/scripts/generate-package-contract.mjs`
  `finiteConditionPartitions`, `certify-contract.mjs`), and the benchmark and
  metric harnesses (`scripts/ecosystem-benchmark/run.mjs`,
  `scripts/certification-metric.mjs`, `make certification-metric`)
- Relation: implements the owner's decision of 2026-09-28 ("certify per
  host"). Closes ADR 0124's open item ("an undeclared host still receives
  `node`-scoped cases"), and gives the rows ADR 0124 scoped to `browser` a
  certification that requests `browser`. Rows it enables:
  `docs/package-contract-v2/audits/2026-09-28-solid-2-rc9-merge-omit-creatememo-creates.md`.

## Context

The certification metric (`phase22/2026-09-28-certification-metric-baseline.md`,
wall 3) found `creates` held open behind `solid-js`' `createSignal` and
`createMemo`. The `createSignal` rows exist, but are scoped to the `browser`
host (ADR 0124), and every package certification requested `["import"]`, so
they never applied.

The problem is broader than two rows. `["import"]` resolves a package's closure
the way no real host does. `solid-js@2.0.0-rc.9`'s `exports` map orders
`worker, browser, deno, node, …, default`, so `["import"]` selects the
`default` arm: the browser bundle `dist/solid.js`. A Node server runs
`dist/server.js`, whose `createSignal`, `createMemo`, `merge` and
`createEffect` differ and can reach `ctx.serialize`. Any dependency with a
`browser` or `node` branch is resolved to its `default` target too. So an
`["import"]` case describes neither the browser's closure (it misses every
`browser` branch outside `solid-js`) nor the server's.

Two facts were already in place:

- **Certification can already run under a host.** `contract certify
  --conditions browser` resolves the whole closure under
  `["browser","import"]`, and the census terminator already replays a scoped
  row's premises against that set (ADR 0124). But `--conditions` is an exact
  list, so it drops the package's own other axes (`solid`,
  `@solid-primitives/source`, `development`). Every package in the metric's
  corpus has one.
- **The consumer's host is declared, never inferred.** The analyzer learns it
  from `--runtime-target browser|node`, `--runtime-condition`, `--rendering` and
  `--runtime-build` (the ESLint adapter passes its `runtime.target` setting as
  `--runtime-target`), which `RuntimeEnvironment::selected_conditions` folds
  into one set. It reads no `tsconfig` `customConditions`, no bundler
  configuration, and no Solid compiler target: the compiler's
  `generate: "dom"` is the analysis default, not a fact about the consumer.
  None of those would be exact. `customConditions` is a type-resolution
  setting, and a Vite build activates `browser` or `node` per environment
  outside any file the checker reads.

## Decision

**A package is certified once per host (`browser`, `node`) as well as host
free. Each host's cases carry the host condition, and so its identity. A
consumer receives only the cases certified for exactly the hosts it declared.
A consumer that declares no host receives only host-free cases. A host nothing
was certified for gets no contract.**

### The host vocabulary

`HostTargetCondition` is now the resolver's whole host axis:
`Browser`, `Node`, `Deno` and `Worker` (`MUTUALLY_EXCLUSIVE_CONDITION_AXES[0]`).
It is still an enum, so a new host is a compile error at every match. Only
`Browser` scopes a negative row. `Node`, `Deno` and `Worker` carry none. The
two `node`-reachable bodies read on 2026-09-28 (`merge`, `createMemo`) both
reach `ctx.serialize`.

### Certification: `--host`

`contract generate`, `contract certify` and the benchmark's `run.mjs` take
`--host <browser|node>`. `finiteConditionPartitions(manifest, requested, host)`
fixes the host axis: every partition carries the host condition. That holds
even where the package's own `exports` map never names a host, because its
closure does. Every other axis is still enumerated, and the per-entrypoint
dedup by branch identity is unchanged. An explicit `--conditions` list stays
exact. With `--host` it gains the host, and it refuses a list that names
another host.

The host is recorded in `certification-inputs.json` (absent for the host-free
run, so those bytes do not move), and a reused proposal must match it. Graph
lane nodes inherit it through the root cases' conditions, as they already
inherited `--conditions`. Nothing on the Rust side changes: the plan's
`import_request.export_conditions` already carries the host into resolution,
the census terminator and the acceptance root.

"Wherever the exports map or its Solid runtime distinguishes them" is
therefore everywhere. Every Solid 2 package's closure carries `solid-js`,
whose `.` distinguishes `browser` from `node` (`dist/solid.js` against
`dist/server.js`). A package whose closure was host-neutral would get equal
cases per host, which is redundant but not wrong. Detecting that case is left
open (below).

### The consumer: equality on the host partition, then the subset rule

`admissible_cases` compares the host conditions a case carries with the host
conditions the consumer declared, and keeps a case only when the two sets are
**equal**. Only then does the existing subset and most-specific rule run
within that partition:

- **No host declared**, which covers every ESLint and Oxlint run and a
  declaration naming only `import`, `solid`, `development` and the like: only
  host-free cases, as before, but now without `node`, `deno` or `worker` cases
  either.
- **`browser` declared**: only `browser` cases. A host-free case is not a
  fallback, even though `["import"]` ⊆ `["browser","import"]`, because it
  resolved another closure. A `node` host likewise receives only `node` cases.
- **`deno` or `worker` declared, or two hosts at once**: no case is certified
  for them, so no contract.

### Measurement

`make certification-metric` certifies the corpus three times: host free
(`run.json`, `metric.json`), `--host browser` and `--host node`
(`run-<host>.json`, `metric-<host>.json`). `metric-hosts.md` puts them side by
side. `CERTIFICATION_METRIC_HOSTS=` restores the single host-free run.
`contract-coverage-census.mjs` refuses to pin a host run, as it refuses a
`--conditions` one.

## Consequences

- **A declared host loses the compiled-in tier until the tier is regenerated.**
  Every bundle in `pkg/contracts/accepted/` today is host free (`["import"]`
  or `["import","solid"]`), so a `--runtime-target browser` or `node` consumer
  now receives none of them. Before this ADR it received them by the subset
  rule, although they resolved another closure. Regenerating the tier from
  per-host runs (the census and consumer-environment runs with `--host`) is
  the lead's step. The bundler carries a case's `exportConditions` already.
- An undeclared host is unchanged for host-free cases. It no longer receives
  `node` cases, which were previously compared at the index.
- The unit expectations in `contract_interface.rs` and
  `accepted_bundles/tests.rs` that encoded "a declared host falls back to a
  host-free case" and "an undeclared host receives a `node` case" moved.
  `a_declared_host_receives_only_cases_certified_for_it` pins the new rule.
- Measured on 2026-09-28 (`make certification-metric`, release build of this
  change, 30-package corpus). The baseline is `caa3d1af`, host free: 43
  exports clean, 3.4 % per-package mean, dialect-silent class 21 exports.

  | host | clean (pooled) | per-package mean | exports | dialect-silent exports |
  | --- | ---: | ---: | ---: | ---: |
  | none | 43 | 3.4 % | 957 | 21 |
  | `browser` | 43 | 3.4 % | 957 | 11 |
  | `node` | 28 | 2.1 % | 866 | 385 |

  - **`browser`.** The `createSignal` and `createMemo` walls clear: the
    dialect-silent class falls from 21 to 11 exports, and the "dialect row
    scoped to the browser host" catch-all falls from 13 to 0. No export turns
    clean, because every one of them has another open domain (graph lane
    unrecorded, recipes). Five `reads` closures are lost. The probe-recipe
    corpus addresses claims by artifact case, so its recipes address the
    host-free cases only, and a `browser` case finds none ("no probe recipe").
    That is an input gap, not a semantic one.
  - **`node`.** The published-graph lane refuses for 19 of the 20 packages
    that use it host free, at the `solid-js@2.0.0-rc.9` graph node: under `[import,node]`, `.`
    runs `dist/server.js`, which defines `action` (and the other server
    bodies) itself, while `types/index.d.ts:1` re-exports the declaration
    from `@solidjs/signals`. The node refuses with "contract identity does not
    match the resolved import". The rows fall back to the plain lane.
    `@tanstack/solid-query` refuses whole, and `@solid-primitives/storage`'s
    `.` publishes no case. Its dialect-silent count is the plain lane's
    `merge`/`createSignal`/`createMemo` walls, which the host-free graph lane
    hides behind "graph lane: unrecorded".

## Still open

- **A host-neutral closure.** A host-free case whose whole closure selects the
  same file under every host could soundly be admitted to every declared host,
  or one certification could stand for all hosts. That needs the receipt to
  record host neutrality, like ADR 0037's neutrality walk for probe launch.
- **`merge`.** Withheld flat because `solid-js`' server builds define their
  own. A `browser` row on `@solidjs/signals` would need a premise that the
  *re-exporting* `solid-js` resolves to a browser build (audit § 2.3).
- **`deno` and `worker`** are not certified. A consumer declaring them gets no
  contract.
- **The `node` graph lane** refuses at `solid-js`' own graph node: the server
  runtime defines what its declaration re-exports from `@solidjs/signals`
  (above). Until that split is bound, `node` certification is plain lane only.
- **Recipes are addressed per artifact case**, so every recipe in
  `scripts/ecosystem-benchmark/probe-recipes/` serves the host-free case alone.
  A host run needs the recipes re-addressed to its claim ids.
