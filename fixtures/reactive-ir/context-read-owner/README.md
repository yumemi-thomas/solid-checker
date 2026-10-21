# context-read-owner

**Claim (ADR 0352).** `useContext` with no owner throws `NoOwnerError`, whatever the context's default. Violations: `useTheme` (called from the handler in `HandlerHook`), `TimerRead` (a timer), `moduleTheme` (module scope), `loadTheme` (after `await`), `DefaultedInHandler` (a defaulted context). Silent: `Panel` (component body), `Themed` (under its provider), `Memoized` (a computation), `Shadowed` (a local binding named `useContext`).

Not claimed: the `ContextNotFoundError` of a context with no provider. tsc: clean against the rc.13 typings. Runtime: `fixtures/runtime-oracle/ctx-owner-read`. Stub: `createContext` and `useContext` copied from published `types/client/core.d.ts`.
