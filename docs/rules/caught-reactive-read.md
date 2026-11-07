# caught-reactive-read

`SC5008` · **warning** · violation

A computation catches the throw of a reactive read and completes with a value.

## What it does

A reactive read is not an `await`. Its throw is how it suspends to a `<Loading>`
boundary, and a `try` in a compute swallows that: rc.21 warns `CAUGHT_REACTIVE_READ`, and
when the source changes later the computation is marked pending and not re-run, so the
catch does not fire again.

The check is per pass: a throw that leaves the pass reports nothing, so
`a(); try { a() } catch {}` is silent (ADR 0637). Proven for a source the body around the
compute created with a literal `async` compute, read on the stack of a sync compute that runs
where it is written, in a `try` whose `catch` has no `throw`, with every other read that may
throw on that pass in such a `try` too. A read is an accessor's call, a Get, `Object.keys`,
`in`, `deep` or another key probe of an async store, or `latest(a)`.

Anything else that may throw in a `try` in a computation, or in a function nothing places, is
uncertifiable: a memo may throw its error, and a value nothing resolves may be pending. A
plain signal cannot throw. A `catch` that is one `throw` is silent: the compute never returns,
and rc.21 checks only a pass that returned.

## How to fix

Branch on readiness with `isPending()`, show ahead with `latest()`, or let the read
suspend. Handle errors with `<Errored>` or `createEffect`'s `error` option.

A `try` around code no call shows reading is uncertifiable too, blocker
`caught-read-unproven` (ADR 0658): a call of a value a caller or a hook handed over, a Get of
a prop or of such a value, and a local literal's accessor.

A call in the `try` of a helper that reaches an accessor which may throw into the pass is the same obligation, at the call; a plain signal cannot.
