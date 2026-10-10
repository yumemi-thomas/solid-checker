# Exact callback returned source identity

Fixture for ADR 0259's createRoot passthrough. `expected-claims.json` names
the semantic assertions; the observed findings are in the snapshot.

`TimerRead`, `CounterRead` and `PaginationRead` mirror the three ledger shapes.
They and `ObjectRead`, `SignalRead`, `MemoRead`, `SelectedRead`,
`SelectedObjectRead`, `WrappedRead` must report SC1001 **violation** at their
component-body accessor calls. The same identities in `TrackedReads`' JSX
must have no strict-read finding. Calling the signal's setter is not a read.

Nonliteral callbacks, multiple/conditional returns, parameters including
dispose, disposal, named/self-escaping callbacks, async callbacks, unknown
factories, promised or disagreeing contracted returns, callback-local escapes,
mutable/default/rest/container bindings and outer escapes/aliases are not
resolved: they keep exactly the evidence they had before the passthrough, and
gain no new obligation. (A blanket obligation on every unresolved root result
added 123 uncertifiable sites over the rc.13 corpus and was dropped.) The
`makeTuple()` obligations are the tuple contract's own, not the root's.
`UnresolvedReads` must have no new proven SC1001 from these withheld identities.
`ShadowedRoot` is clean of new strict reads: the same-spelled local factory
returns a plain function. `shadowedBinding` stays unresolved: its returned
`read` is a different declaration from the inner contracted accessor.

The Solid stubs are copied unchanged from `write-scope-roots-rc9`, including
its byte-faithful createRoot, signal/memo signatures and dependencies. JSX's
container declaration is reduced, as in `package-own-tracked-read-consumer`;
it is not authority for reactivity. The synthetic package declarations describe
this fixture's stated values; they are not substitutes for published primitive
typings. The package.json bytes, artifact/closure hashes and out-of-band
authorize-contract.json record follow `package-own-tracked-read-consumer`;
only the export catalogue and stated graphs differ. There is no obsolete
accepted-contracts.json catalog. The accessor return deliberately uses the
timer contract's min:0/max:many cardinality; binding its returned value must
not strengthen an operation's execution count.
