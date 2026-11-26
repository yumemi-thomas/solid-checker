# refresh-prop-target

**Claim (ADR 1174).** `refresh(props.name)` whose target the caller census resolves (ADR 0919 `passed_on_every_site`: every
caller enumerated, each passing one exact source and nothing else) to one refreshable source (an accessor, or a store not of the
value form), on a props parameter that is current with only views attached (or a never-written slot of a destructured one), is a write of that source, placed by the call's own role
with the source's `ownedWrite` option. rc.21's `refresh` checks the target's brand and then, in an owned scope, throws
REACTIVE_WRITE_IN_OWNED_SCOPE (`@solidjs/signals` `dist/dev-shared.js` `markRefresh`). Before, ADR 0509's branch left every such
write unplaced (SC2001 `function-run-unplaced`) although the call sits directly in the compute.

Violations: `MemoParent` (in a memo), `EffectParent` (in an effect's compute), `DestructuredParent` (a never-written slot of a destructured props parameter). Clean: `HandlerParent`. Uncertifiable: `TwoCallers`
(two sources), `SpreadParent` (a spread caller), `WrittenParent` (the component writes `props.m1`). Chrome:
`runtime-oracle/refresh-prop-target`. Stub: reduced rc.21 signatures (`refresh` takes an accessor here for the brand).

**ADR 1230.** A component that hands the target's component its own props whole passes what its callers pass (ADR 1173's
checked forwards, `passed_through_forwards`): `ForwardedParent` (one forwarder) and `TwoLevelParent` (two) are violations;
`OverrideParent` (the forwarder sets `m1` after its spread) and `ForwardTwo` (the forwarder's callers pass two memos) stay
uncertifiable.
