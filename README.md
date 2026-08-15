# Solid TS Facts

Type Facts provides compiler-independent semantic facts about a configured
TypeScript project. This repository owns both sides of the protocol:

- `cmd/solid-typefacts` is the TypeScript-Go producer.
- `crates/typefacts` is the Rust model, deterministic-CBOR codec, and retained
  session client.
- `schema` contains the frozen lifecycle schemas through v9 and codec limits.
  The producer reports that schema's digest in its startup handshake, and the
  client rejects a producer whose digest, protocol version, or build id differs.

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

The compiler-derived contract facts and their TypeScript API mapping are
documented in [docs/compiler-semantic-facts.md](docs/compiler-semantic-facts.md).
The corresponding solid-checker heuristic removals are listed in
[docs/migration-solid-checker.md](docs/migration-solid-checker.md).

Tagged releases publish `solid-typefacts` binaries for Linux, macOS, and
Windows on x64 and arm64 where supported, plus a `SHA256SUMS` manifest.
