# Mandatory versus optional first compute

Pins the distinction ADR 0244 relies on for `createBodyCursor` and
`capitalize`. A tracked compute stated `min: 1` runs during the call, so a
write in it is a proven `reactive-write-in-owned-scope` violation. A `min: 0`
compute (`possibleTracked`, `lazyTracked`) is not upgraded: its write stays
unproven. Its reads are clean under ADR 0244, because when it runs, it runs
tracked under its created owner.

The contract is accepted out of band through `.solid-checker/authorize-contract.json`.
`tsc --noEmit` is clean on this directory.
