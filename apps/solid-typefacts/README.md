# Solid Type Facts

Type Facts provides compiler-independent semantic facts about a configured
TypeScript project. This repository owns both sides of the protocol:

- `apps/solid-typefacts` is the TypeScript-Go producer.
- `rust/crates/typefacts` is the Rust model, deterministic-CBOR codec, and retained
  session client.
- `schema/typefacts-*` contains the active lifecycle schema and codec limits.
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

The imported design history is retained under `docs/typefacts/`; repository
integration and release ownership now belong to solid-checker.
