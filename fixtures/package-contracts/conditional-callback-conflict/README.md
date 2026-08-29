# Conditional callback schedules

`schedule(callback)` invokes inline in the `development` artifact and through
`queueMicrotask` in the default artifact. Both are known positives, but they
belong to different exact artifact cases.

The stable-v1 proposal represents each callback as an operation with
independent schedule, tracking, ownership, trigger, and cardinality axes. It
does not union mutually exclusive callback rows into an environment-unaware
summary. Exact selection returns one case; unresolved selection monotonically
joins both as possible behavior and guarantees neither schedule.

`expected.json` pins the two artifact identities and operation graphs.
`expected-proposal.json` pins the recursively open domains without opening the
known callback schedule. The intra-artifact companion is
`multi-role-callback-parameter`, where one parameter reaches incompatible
execution sites in a single case and only that callback leaf remains open.
