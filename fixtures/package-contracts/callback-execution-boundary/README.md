# `execution: "inline"` is a promise about the export, not about the call

A callback row saying `execution: "inline"` tells a consumer the export invokes
the callback **before it returns** — that is what makes it safe to pass a
tracked reader, and what makes an owner available. Two rungs of the derivation
used to make that promise from evidence that does not support it:

- the lexical execution role. A capitalized function's body classifies as
  `UntrackedRendering`, which maps to `inline`. It describes the region the
  call is *written* in, and says nothing about whether that region runs.
- the call shape. `direct_callee` says the callee is a plain identifier, which
  is a property of the call, not of its schedule.

Neither notices a closure boundary between the call and the function that
declares the parameter, so both of these published `inline`:

- `Escaping` writes `onData(1)` inside an arrow handed to `schedule`. What
  `schedule` does with that arrow is exactly what the analysis cannot see: it
  may invoke it later, once, or — as here — never.
- `Returned` writes `onData(1)` inside a closure it hands back. Nothing invokes
  it during the call at all.

Both now write no row, and the unknown-callback obligation opens the
`callbacks` sentinel instead. A consumer that demands the callback claim fails
closed; one that does not is no longer told a promise the package does not
keep.

`Inline` is the negative that keeps the derivation useful: `onData(1)` is
written directly in the body of the function that declares `onData`, so the
invocation really is proven and the row survives with its real execution mode.

The boundary is read off the AST rather than off the summary-node universe: an
expression-bodied arrow (`() => onData(1)`) is a real function boundary, and
the node-index form of this test called exactly that shape inline.

`Escaping` additionally carries `reactiveReads`/`returns` unknown, from the
unresolved `.getThing` dispatch inside `schedule` — an independent obligation
that this fixture does not otherwise exercise.

## What this does not yet prove

`schedule` never uses its second parameter, so the honest answer for `Escaping`
is *no row at all* — a proven negative. Proving that needs an interprocedural
"this parameter is never invoked" summary the generator does not compute, so
the fixture pins the fail-closed sentinel instead. Recorded in
docs/precision-backlog.md.
