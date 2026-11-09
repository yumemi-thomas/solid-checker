# Leaf call in a stepped generator (ADR 0929)

`(function* () { … })().next()` runs to its first `yield` in place, an
IIFE: judged at the call, V (`Settled`) or as a direct `flush` (`Reaches`,
`Wrapper`); U if a `yield` may stop first (`MayStop`). Clean: past the
`yield`, `return()`. `Held` stays U.
