# Reads after an unconditional await (ADR 0323, review round 9)

A signal read after `await 0` in an async callback that a project callee calls
in its body was a proven `SC1001` violation. The synchronous prefix of the
callback runs in the render window; what follows an await its function reaches
on every path runs on a later task, where `strictRead` is already restored
(`@solidjs/signals@2.0.0-rc.13` `dist/dev-shared.js:5801-5885`: `untrack(fn,
label)` sets it for the synchronous `fn()` and restores it in `finally`).
Neither a reporting role nor a tracking scope describes that read, so the
role is now "no claim" for every reporting role when
`follows_unconditional_await` holds (statement-level awaits only: the same
fact ADR 0320's dispatch obligations use).

Reported (before-await): `BeforeAwait`, `StoreBeforeAwait`,
`PropsBeforeAwait`, the first read of `AsyncIife`. Silent: `AfterAwait`,
`StoreAfterAwait`, `PropsAfterAwait`, the second read of `AsyncIife`. Still
reported, because the await may be skipped and the read may run in the window:
`SkippableAssign` (`x ??= await p`), `SkippableOptionalCall` (`f?.(await p)`),
`SkippableLabelled` (`break skip`). The tree passes `tsc --noEmit` against the
published rc.13 declarations (copied unchanged from
`standard-runtime-configuration`).
