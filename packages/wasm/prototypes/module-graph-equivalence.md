# Module-graph equivalence prototype

`bun run --cwd packages/wasm prototype:module-graph` is manual evidence. It is
not a gate, a CI step, or a maintained WASM provider.

## What it does

It builds `apps/solid-typefacts/prototypes/typefacts-reactor` twice from the
same source:

- as a native stdin/stdout command;
- as a `wasip1` reactor.

It uses the installed Go toolchain and module cache only (`GOPROXY=off`,
`GOTOOLCHAIN=local`). Each case copies `testdata/forms` into a fresh,
realpath-resolved scratch directory, outside the repository. The default is
`/tmp/sc-scratch.noindex/p5-canonical-module-graph`; set `MODULE_GRAPH_SCRATCH`
to override it.

Each case then:

- freezes the bytes;
- answers one request natively from disk;
- answers the same request from a fresh WASI instance whose
  `@napi-rs/wasm-runtime` memfs holds exactly those bytes at the same absolute
  paths;
- compares the raw responses.

Responses are compared as raw bytes, either whole or as a top-level field
located by byte range. They are never parsed and re-encoded for comparison.
The native command's single framing newline is the only byte removed.

## Required cases

- **Positive:** a semantic demand (`Symbol` at `src/alias-use.ts:[53,60)`)
  and the full graph demand. Before the outputs are compared, it asserts
  exactly:
  - the JS `require`, JSDoc, ambient, import-type, export-from,
    import-equals, dynamic, alias, npm and unresolved rows;
  - the answered entity at that exact location, with a resolved, nonempty
    symbol.
- **No graph:** the request without `moduleGraph`. It must carry the same
  answered entity and the same table bytes as the graph run.
- **Zero demand:** `moduleGraph: {}`.
- **Omission:** one demanded source is missing from memfs only. It must become
  an explicit unknown import path and change the graph.
- **Changed source:** changed bytes change that file's digest only.
- **Failure:** a project that cannot open fails with no table and no graph.

## Recorded-only cases

The `symlink` and `case` cases are recorded, not required. They show whether
filesystem identity agrees, and an explicit discrepancy is recorded as one.

## What it does not show

- **Entity and symbol fields:**
  - The reactor reads entities through `FactTable.RangeEntities`, the owner's
    canonical retained-run projection.
  - JSON drops the entity fields tagged `json:"-"`: `PrimitiveValueDomain`,
    `PrimitiveLiteralCandidates`, `Constructability` and
    `RuntimeBindingKind`. The packed transition carries them.
  - `symbols` is `null` by the owner's design, because Rust performs the
    symbol closure.
  - For a nonempty demand, the no-graph bytes differ from the base: the base
    lost the answer.
- **Codec and lifecycle:** it does not exercise the lifecycle CBOR codec,
  generation enforcement, `Session::module_graph` or any Rust consumer. The
  native side calls the same `DemandClosure.ModuleGraph` through this
  prototype's JSON driver.
- **Filesystem coverage:** it says nothing about config overlays, project
  references or duplicate-install redirects, browser workers, or general
  filesystem fidelity.
- **Authority:** it does not show that a host supplies complete or authentic
  projects.

Outputs, built images and the frozen trees stay under the scratch directory.
