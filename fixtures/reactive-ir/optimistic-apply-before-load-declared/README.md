# SC2009: a declared loadingValue answers first (ADR 1175)

`createOptimistic(async () => 1, { loadingValue: 0 })`: the setter has an answer to be optimistic over, Chrome is silent, and the
proof (which admits no options argument) does not apply.
