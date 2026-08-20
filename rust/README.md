# Rust analysis foundations

This workspace is the production analysis backend. It includes the checker and
the CLI. `ARCHITECTURE.md` describes the crate layout: dialect-independent
infrastructure under `crates/`, Solid-version-specific rules and compiler
adapters under `dialects/`.

Fact ownership is deliberately split:

- `crates/solid-facts`: the fact model. Its `core` module owns source
  identity, generations, hashes, and byte spans; `ast` owns parser-derived
  source structure from one Oxc AST walk; `compiler` owns Solid compiler
  execution roles (`ExecutionMap`) and the `CompilerFactsProvider` seam; the
  crate root validates and joins the domains without exposing either Oxc or
  TypeScript-Go nodes;
- `typefacts`: checker-derived facts and the retained producer session, from
  [solid-ts-facts](https://github.com/yumemi-thomas/solid-ts-facts);
- `crates/solid-facts-backend`: orchestration, retained caches, certification
  snapshots, contracts, and the CLI;
- `crates/solid-dialect`: the Solid vocabulary seam — the version-specific
  answers (primitives, callback positions, boundary tags) the engine asks for
  instead of knowing;
- `crates/solid-reactive-ir`: native analysis, producing the reactive program
  IR and the dialect-neutral `Finding` model;
- `dialects/solid-v2/rules` and `dialects/solid-v1/rules`: each version's
  rule catalog and finding construction (32 rules for 2.0, 22 for 1.x);
- `dialects/solid-v2/compiler` and `dialects/solid-v1/compiler`: the
  dom-expressions compiler and its Solid 1.x fork, each adapted to the
  `CompilerFactsProvider` seam.

The AST package contains no regular expressions. TypeScript facts contain no
syntax-discovery fallback. Both choices are architectural constraints: Oxc owns
structure and TypeScript-Go owns checker semantics.

The production Rust path has one live seam: Rust to the `solid-typefacts`
producer. Oxc AST facts and Solid compiler facts run in-process, the latter via
the `dom-expressions-compiler` crate's semantic trace.

The producer and its Rust client both live in the `solid-ts-facts` repository.
`TypeFactsSession` is the checker-side adapter over `typefacts::Session`, which
owns the process, framing, handshake, retained demands, delta application,
cancellation, and restart-and-replay. `scripts/build-typefacts.sh` builds the
producer from the revision `Cargo.toml` pins the client to, because the startup
handshake rejects any other pairing.

The integration tests launch the real producer on the tracer fixture and join
its output with the in-process Oxc and compiler facts.

The Rust-led path sends Oxc-derived identifier locations as authoritative
closure seeds. When those seeds are present, the Go closure builder bypasses
its legacy regular-expression discovery. Oxc parsing is bounded by
`available_parallelism`; output is restored to source order before joining.
AST facts are cached by path and source hash, and compiler facts by path,
source hash, and compiler options.

`NativeIncrementalSession` retains the TypeFacts session, current source
overlays, generation, and both caches. An update invalidates only
changed/deleted paths; unchanged file facts survive and output remains sorted.

`solid-checker-rust` is the diagnostic CLI. It accepts `--project`,
loads the Oxc Solid compiler as an in-process Rust crate, and uses
the sibling `solid-typefacts` executable automatically; `SOLID_TYPEFACTS_BIN`
or `--typefacts` can override it.
TypeScript-Go supplies the configured project source set, so tsconfig
include/exclude and project resolution are authoritative rather than
reimplemented by a directory walk.

The CLI defaults to text output. `--format json` emits the stable certification
snapshot (`status`, findings, package summaries, and metrics), `--certify`
returns exit code 1 unless the status is `certified`, repeatable `--contract`
flags override discovered contracts, and `--validate-contract` validates a
contract and its artifact hashes without opening a project.

Implemented rule slices are:

- `strict-read-untracked`;
- `reactive-write-in-owned-scope`;
- `action-called-in-owned-scope`;
- `cleanup-in-forbidden-scope`;
- `primitive-in-leaf-owner`;
- `flush-in-forbidden-scope`;
- `missing-effect-function`;
- `sync-node-received-async`;
- `refresh` writes flowing through the owned-scope rule (the refresh/affects
  *target* rules were removed: `Refreshable<T>` is the brand as a type, so
  every invalid target is already a TypeScript error);
- `primitive-in-directive-application`, covering owner-attaching primitives
  (value-form state constructors are exempt) in direct application callbacks
  and in closures returned through forwarded directive factories;
- `no-owner-effect`, `no-owner-cleanup`, `no-owner-boundary`, and
  `no-owner-settled-cleanup`, using a fixed-point owner-context graph across
  components, roots, helpers, effect phases, events, and leaf owners;
- `pending-async-untracked-read`, `pending-async-forbidden-scope`, and
  `async-outside-loading-boundary`, with TS-Go async-result provenance and Oxc
  JSX dominance through aliased boundaries, components, and boundary wrappers;
- `reactive-read-after-await`, using TS-Go's dominance-proven
  `callsAfterAwait` facts rather than source-order guesses;
- component props reads, aliases, Solid `merge`, and
  `no-destructure`, plus `component-returns-conditionally` for
  reactive return-shape guards, with Oxc binding/member shapes and checker
  identities.

The slices above are the engine's analyses under Solid 2.0 vocabulary and
the `solid-v2-rules` catalog. The Solid 1.x dialect projects the same IR
onto its own 22-rule catalog (`v1/<rule>` names): the engine slices under
1.x vocabulary plus the eslint-plugin-solid file-local surface (imports, JSX
hygiene, structural preferences, and the decomposed `reactivity` rules). The
dialect is auto-detected from the project's resolved `solid-js` version;
`--dialect` overrides it.

Oxc discovers bindings, options, calls, callback nesting, and function graphs;
TS-Go joins canonical symbols across imports; ExecutionMap classifies tracked
JSX and compiler-managed callbacks. Cleanup return shapes come directly from
Oxc expression kinds; TS-Go resolves locally or remotely declared functions
and call return types. Function summaries are instantiated once per owned
root and at rendering call sites. The fixed-point summaries cover cross-file
helpers, callback parameters, generics and overload implementations, recursive
SCCs, returned closures, and store paths while preserving Go solver
multiplicity through cycles.

```sh
SOLID_TYPEFACTS_BIN=bin/solid-typefacts \
cargo +1.97 run --manifest-path rust/Cargo.toml --bin solid-checker-rust -- \
  --format json --certify \
  --project fixtures/reactive-ir/tracer/tsconfig.json
```

Set `SOLID_CHECKER_TIMINGS=1` to emit nanosecond stage timings on stderr. Oxc AST
and Solid compiler facts are produced in parallel per source; deterministic
source order is restored before the TS-Go closure is joined.

`make package` creates one install tree containing `solid-checker` and the
matching `solid-typefacts` helper plus a checksum manifest. CI builds and smoke-tests that layout on Darwin and Linux
arm64/amd64 and Windows amd64. Tagged releases publish each layout as a
platform-constrained optional npm package alongside the portable launcher.
The `solid-checker-wasm` workspace crate exposes the same in-process analysis
pipeline through napi-rs on `wasm32-wasip1-threads`; its host supplies sources
and TypeFacts directly instead of spawning the native TypeFacts service.
