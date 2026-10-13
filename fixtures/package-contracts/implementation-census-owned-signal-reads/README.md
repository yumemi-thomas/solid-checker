# implementation-census-owned-signal-reads

The generator's half of ADR 0146
(`docs/adr/0146-a-described-callable-may-read-a-signal-its-export-created.md`):
an export whose reactive analysis describes the return as an accessor, and
whose every value-carrying completion is that accessor or a function literal,
proposes `returns` closed over a described callable that reads a signal the
export owns (`reads: [owned-signal]`), instead of the reactive accessor output
no census could decide.

| export | proposal | what the census decides |
| --- | --- | --- |
| `createCounter` | `returns: [read-value]` | the returned value traces to `createSignal(0)`'s slot 0 |
| `createReader` | `returns: [read-value]` | the literal's one call is a read of that accessor, and it hands back what it read |
| `createDoubled` | `returns: [plain]` | `count() * 2` is a number by grammar |
| `createFrom` | `reads: [owned-memo]`, `returns: [read-value]` | ADR 0175: `createSignal(initial)` over the caller's value may take the writable-memo path, so the proposal is the computed read, which is true of both paths |

This fixture pins generation only. The `solid-js` stub under `node_modules` is
there for dialect selection and so the reactive analysis resolves
`createSignal`; it is not the audited archive, so no certification here could
use it, and the census's own premises -- the audited dialect declaration, the
arguments' grammar, the read -- are pinned on synthesized transcripts in
`type_facts.rs`'s `an_owned_signal_is_witnessed_only_for_an_inert_audited_accessor`.
