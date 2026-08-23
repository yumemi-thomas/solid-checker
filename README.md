# Solid TS Facts

Type Facts provides compiler-independent semantic facts about a configured
TypeScript project. This repository owns both sides of the protocol:

- `cmd/solid-typefacts` is the TypeScript-Go producer.
- `crates/typefacts` is the Rust model, deterministic-CBOR codec, and retained
  session client.
- `schema` contains the single active lifecycle schema (V1) and codec limits.
  V1 carries the latest semantic vocabulary. The producer reports its digest
  in the startup handshake, and the client rejects a producer whose digest,
  protocol version, or build id differs.

The Rust client takes an explicit producer path. It does not inspect
environment variables, search `PATH`, or assume a consumer's packaging layout.

```rust
use typefacts::{AnalysisDemand, Producer, Session};

let producer = Producer::at("/path/to/solid-typefacts");
let mut session = Session::open(producer, "/project/tsconfig.json", Vec::new())?;
let facts = session.analyze(&AnalysisDemand::default())?;
session.update(changes)?;
session.close()?;
# Ok::<(), typefacts::SessionError>(())
```

A session also answers for the resolved module graph of the open generation —
the files the TypeScript program actually included, and where each module
specifier resolved. It is a read of the retained program: it advances no
generation and leaves a materialized analysis untouched.

```rust
use typefacts::ModuleGraphDemand;

let graph = session.module_graph(&ModuleGraphDemand::with_all_imports().with_packages())?;
for import in graph.imports_from("/project/src/index.ts") {
    println!("{} -> {}", import.text, import.resolved_path);
}
# Ok::<(), typefacts::SessionError>(())
```

## Development

```sh
make test
```

The compiler-derived contract facts and their TypeScript API mapping are
documented in [docs/compiler-semantic-facts.md](docs/compiler-semantic-facts.md).
The corresponding solid-checker heuristic removals are listed in
[docs/migration-solid-checker.md](docs/migration-solid-checker.md).

Tagged releases publish `solid-typefacts` binaries for Linux, macOS, and
Windows on x64 and arm64 where supported, plus a `SHA256SUMS` manifest.
