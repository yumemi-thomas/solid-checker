# forwarded-async-read

**Claim (ADR 1155 r3).** ADR 0200's forwarding gave a read in a literal a project or read-package function forwards to a
computation's slot the tracked role, and the pending rules took that role for a render read. Bundle o's census found the result:
SC5003 a violation where Chrome is quiet or reports another code (also on main b06a4e71a). Now:
- such a read takes the role, the computation and the leaf owner of a read written at the wrapper's call of its parameter in the
  slot's callback (`callee_roles::forwarded_async_site`);
- a creation forwarded only into a leaf meets the leaf's refusal first.

| component | outcome |
|---|---|
| `MemoApp` (forwarded to createMemo's compute) | uncertifiable, no SC5003 claim (Chrome quiet) |
| `TrackedApp` (forwarded to createTrackedEffect's) | SC5001 a violation (Chrome PENDING_ASYNC_FORBIDDEN_SCOPE), as `DirectTrackedApp` |
| `SyncApp` (`createEffect(async…, …, { sync: true })` forwarded into the leaf) | SC3001 a violation (ADR 1155 r4, `forwarded-leaf-operation`); SC7002 uncertifiable `creation-throws-first` (main: a violation; Chrome PRIMITIVE_IN_FORBIDDEN_SCOPE) |
| `DirectMemoApp`, `DirectTrackedApp` | unchanged: the twins this placement mirrors |

Chrome: `fixtures/runtime-oracle/forwarded-async-read`. The census grid holds the package form (`barm-fwdleaf-{memo,tracked}`,
rediscovery.json). Stub: `forwarded-leaf-write`'s.
