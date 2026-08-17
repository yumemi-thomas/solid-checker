# ADR 0009: v6 makes Rust the semantic-closure owner

> Historical record. Superseded as active protocol guidance by
> [ADR 0013](0013-v1-call-result-runtime-value-domains.md); the repository now
> ships one lifecycle schema, V1.

## Decision

Schema v6 preserves fact semantics while moving the closure and retained
indexes across the process boundary:

- Rust's `FactTable` is the canonical expanded source/entity/file table.
- Rust derives symbol roots from those rows, runs the alias fixed point, owns
  reference-tier membership, and retains the canonical symbol/reference table.
- Go is a batched TSGo oracle. It extracts changed path contributions and
  answers alias, declaration, changed-reference, and canonical-reference
  queries while one checker lease is live. Returned symbol rows are not
  retained in Go.
- Symbol oracle results reuse the packed fact-table dictionary codec. They
  cross the process seam once without a second complete CBOR object graph.
- A v6 delta names exact candidate paths and sends unconditional replacements
  or removals. Rust compares those candidates with its retained table before
  publishing `TableChanges`.
- Stable updates query only symbols declared by affected paths and reference
  IDs named by TSGo's exact changed-reference manifest. Reference evidence is
  filtered to affected paths; Rust patches those runs without cloning or
  replacing high-fanout symbol rows.
- If roots, aliases, or path semantics invalidate the stable proof, Rust
  conservatively reruns its full closure against the oracle.

The producer defaults to the frozen v5 adapter when invoked directly. The v6
Rust client selects v6 explicitly with `-schema=6`; each mode reports its own
frozen schema digest in the startup handshake.

## Consequences

The complete semantic result and reference index are retained once, in Rust.
Go still owns the TypeScript-Go program, ASTs, per-file semantic contribution
memo, and suppression/dependency proof needed for incremental TSGo extraction;
moving those compiler-bound structures would require replacing TSGo itself,
not merely changing the process protocol. Cached analysis remains client-local.
Incremental correctness comes from exact affected-path and changed-reference
evidence, with conservative full closure as the fallback.
