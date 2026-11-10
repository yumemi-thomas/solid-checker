# Setter or action handed to a caller (ADR 0929)

`run(setCount)`, exact `run` calling its parameter on every completion,
writes at the call: V in a memo (`Declared`, `Local`,
`Action`); U on some paths (`SomePaths`). Clean: called later or never
(`Later`), or in a handler (`Handler`).
