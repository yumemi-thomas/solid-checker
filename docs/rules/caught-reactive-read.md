# caught-reactive-read

`SC5008` · **warning** · violation

A computation catches the throw of a reactive read and completes with a value.

## What it does

A reactive read is not an `await`. Its throw is how it suspends to a `<Loading>`
boundary, and a `try` in a compute swallows that: rc.21 warns `CAUGHT_REACTIVE_READ`, and
when the source changes later the computation is marked pending and not re-run, so the
catch does not fire again. Proven for an async source the same body created with a literal
`async` compute, read first in the block, in a compute that is not `async` and a `catch`
with no `throw` (ADR 0637). A read of a memo, which may throw its error, or of a source
nothing proves pending, is uncertifiable. A plain signal cannot throw.

## How to fix

Branch on readiness with `isPending()`, show ahead with `latest()`, or let the read
suspend. Handle errors with `<Errored>` or `createEffect`'s `error` option.
