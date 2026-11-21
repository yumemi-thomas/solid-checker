# forwarded-leaf-operation

**Claim (ADR 1155 r4).** A call the leaf refuses (a creation, `onCleanup`, `flush()`) written directly in a literal ADR 0200's
proof forwards only into a leaf callback (`hand(fn) { createTrackedEffect(() => { fn(); }); }`, no `arguments[k]()`, no
`onSettled`) runs only on that callback's stack. SC3001 reports it where it is written (`callee_roles::forwarded_leaf_call`).
The literal and the leaf callback must be neither `async` nor generators, and the site must be in the literal's own body.

| component | outcome |
|---|---|
| `SyncApp`, `MemoApp`, `CleanupApp`, `FlushApp`, `NamespaceApp`, `MemberApp` (`api.hand`) | SC3001 a violation (before: uncertifiable `leaf-operation-unaccounted`) |
| `SignalApp` (value-form state), `ShadowedApp` (a local `createMemo`), `MemoComputeApp` (a memo's compute owns children) | no SC3001 row |
| `NestedApp` (a function the literal only creates), `AsyncLiteralApp` (past an `await`) | no violation |
| `BothApp` (`arguments[0]()`), `SettledApp` (`onSettled`), `RelayApp` (handed on by name) | uncertifiable, as before |

Chrome: `fixtures/runtime-oracle/forwarded-leaf-operation`. It throws at each violation (PRIMITIVE_IN_FORBIDDEN_SCOPE,
CLEANUP_IN_FORBIDDEN_SCOPE, the flush guard) and at `BothApp`, `SettledApp` and `RelayApp`, and nowhere else. Stub:
`forwarded-async-read`'s.
