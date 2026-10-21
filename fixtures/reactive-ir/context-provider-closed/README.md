# context-provider-closed

**Claim (ADR 0349).** In a closed program, a read under an owner of a context created with no default throws `ContextNotFoundError` when no provider supplies it. A context is provider-free when no `<Ctx value>` element and no unnamed use (an escape) reaches its binding.

Violations (`missing-context-provider`): `Panel` (a component body), `Memo` (a computation a component creates), and `readRemote` in `remote.ts` (read under its caller's owner, across files).

Not this rule: `HandlerRead` reads with no owner, which is `missing-owner` (SC4001).

Silent: `Provided` and `Reader` (a provider above the reader), `Defaulted` (a default makes the read a value), and `Escaped` (stored in `holders`, so a holder may provide it).

Premises: the closed program is the `private` manifest. Runtime: `fixtures/runtime-oracle/ctx-provider-missing`.
