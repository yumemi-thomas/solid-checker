# impostor-symbol-key

ADR 0875: a key that is exactly `Symbol(…)` or `Symbol.for(…)`, or a listed well-known symbol, names no census member, so the
sites below raise no `*`. `items().forEach` and `pending().then` keep their standard resolution: the writes in the callbacks are what they
are without an impostor (`forEach` a violation, `then` a fresh stack).
