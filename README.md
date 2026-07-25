# Type Facts

Type Facts provides compiler-independent semantic facts about a configured
TypeScript project. This repository owns both sides of the protocol:

- `cmd/solid-typefacts` is the TypeScript-Go producer.
- `crates/typefacts` is the Rust model, deterministic-CBOR codec, and retained
  session client.
- `schema` contains the frozen v1/v2 wire schemas and codec limits.

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

## Development

```sh
make test
```

Tagged releases publish `solid-typefacts` binaries for Linux, macOS, and
Windows on x64 and arm64 where supported, plus a `SHA256SUMS` manifest.
