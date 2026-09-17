# Independent artifact-case preparation in the published dependency graph

Composing a dependency's receipt into a dependent means preparing them in one
transaction: the graph lane resolves every artifact case, walks each root's
dependency closure, acquires each node's published archive, and generates each
node's proposal dependency-first, so a node's contract exists before anything
above it is generated against it. Preparation was all-or-nothing across that
whole set. One node that could not be resolved, located, acquired or generated
threw, and the lane was abandoned for every artifact case in the transaction.

Sharing the evidence is the point of preparing cases together. Sharing the
failures is not. The measured corpus paid for that in four different ways at
once: `@solidjs/start@2.0.3` on a `crossws` install missing above `h3`'s
declarations, `solid-devtools@0.34.5` on `@babel/core`'s
`./babel-7-helpers.cjs`, `@solidjs/web@2.0.0-rc.3` and `solid-js@1.9.14` on
`node:async_hooks`, which is a Node builtin and can never carry a package
receipt. In each the reason belonged to a handful of the row's cases and
refused all of them.

Preparation now isolates. Each artifact case is prepared independently; a node
that refuses is recorded by name with the stage that refused it, and the
refusal reaches every node generated against its contract — its dependents, and
only those, because a dependent generated without the dependency it names is
exactly the dependency-blind proposal this lane exists to replace. A case whose
graph reaches a refused node is refused by naming that node; the rest form the
graph. Nodes no surviving root reaches are dropped before acquisition, so a
refused case does not spend the transaction's budget or let its own failures
refuse a graph it is not part of. The lane is abandoned only when nothing
survives, which is when the caller's proposal is the better answer anyway.

The retained cases are a floor. Recovery prepares the proposal's own cases
alongside the dependency frontier, and a retained case is one the proposal lane
already generated and would publish. Here — unlike at certification time, where
every case is still unproved and [ADR 0070](0070-independent-prepared-set-selection.md)
weighs one wager against another — the comparison is certain: a retained case
this graph cannot prepare is one this lane certainly will not publish. Taking
the lane anyway would trade a covered case for a chance at a frontier one. So a
dropped retained case abandons the graph and the row publishes its proposal
exactly as it would have without the lane. That rule is what the measurement
found: without it `@solidjs/start` published one entrypoint where its proposal
published ten.

This replaces the preparation-level case search that grew a retained prefix one
case at a time, capped at 32 cases. That search cost one full preparation per
case, could not isolate anything when the baseline itself failed — which is
what happened to every row above — and its cap excluded exactly the largest
rows. Isolation needs one preparation pass and no cap, and it attributes a
failure to the exact node rather than inferring it from which selection broke.

The audit records the requested case count, the surviving one, and every
refused case with its coordinates, stage and reason, so a case this lane
dropped is never indistinguishable from one it never had.

This changes orchestration only. Contract, receipt, proof and trust interfaces,
the protocol and every snapshot are untouched, and no node's evidence is
weakened: each surviving case still carries its complete graph, and the native
transaction still rebuilds and authenticates all of it.
