# ADR 0161: A may-register owner requirement is a proof obligation

- Status: accepted and implemented (2026-09-29); written with the
  implementation.
- Date: 2026-09-29
- Owners: the consumer's owner-requirement projection
  (`solid-reactive-ir` `project_owner_requirements`, `owners.rs`), the
  generator's owner-requirement funnel (`solid-facts-backend` `main.rs`
  `generated_owner_requirements_by_symbol`, `inferred_contract.rs`
  `owner_requirement_operation`, `solid-facts` `ast::unconditional_calls`) and
  the certifier's operation-cardinality census (`type_facts.rs`).
- Relation: reads the producer's `ImplementationCall::unconditional` (ADR
  0152, protocol 71) under the strict floor ADR 0159 made a lower bound. No
  protocol change, no contract-vocabulary change: `count.min` already exists.

## Context

An owner-requirement operation of a package contract (a `cleanup` or
`compute` that requires an owner, sourced from the ambient owner at call)
carries a per-call count. The generator wrote `min: 0` for every one, because
the census could prove nothing tighter than `0..many`, and the consumer
projected every such operation into `SC4001 missing-owner` as a **violation**
at an unowned call site.

`min: 0` says the call *may* register. Most published @solid-primitives
exports register conditionally: nearly every one starts `if (isServer)
return;`, and others register only under an argument test.
`@solid-primitives/mutation-observer@3.0.0-next.2` `createMutationObserver`
registers `onSettled(start); onCleanup(stop);` only `if (isSupported)`, and
`isSupported` is `!isServer`; `@solid-primitives/timer@1.4.5-next.1`
`createTimer` returns before anything under `isServer`. Under the node host
such a call registers nothing, so an unowned call there does not misbehave at
all -- yet it was reported as a proven violation.

## What the rule says

The finding kind is defined in CONTEXT.md: a **violation** is where "the
analyzer proved the code misbehaves at runtime", and **uncertifiable** is "a
proof obligation the analyzer could not resolve". The rule page
(docs/rules/missing-owner.md) states the misbehaviour: "An owner-requiring
operation executes without a reactive owner", and classifies its findings as
"violation (uncertifiable, reported as an error, when required ownership facts
are unresolved)". Its own precedent for an operation that may or may not
execute is the later run of a `createRenderEffect` apply callback, "reported
as uncertifiable, because whether a later run happens depends on its
compute's sources changing".

A `min: 0` operation is that case: whether the operation executes at all is
what the contract does not state. So an unowned call of a may-register export
is uncertifiable, and a violation needs an operation that executes on every
call.

## Decision

- **The consumer follows the lower bound.** `ContractOwnerRequirement` carries
  `guaranteed`: some owner-requirement operation of that kind has
  `count.min >= 1`. An unowned call of a contract requirement that is not
  guaranteed is reported `SC4001` uncertifiable (severity error, as every
  uncertifiable is); a guaranteed one stays the proven violation. It is never
  dropped. Operations of one kind merge by `or`: one guaranteed operation is
  enough. The through-contract status is recorded on the requirement so the
  generator never re-derives a guarantee from a consumer's view.
- **The generator proposes `min: 1` only for an unconditional primitive
  call.** For an owner requirement a function reaches through a dialect
  primitive call it writes itself (not through another contract), the
  generator asks `solid_facts::ast::unconditional_calls` whether that call
  runs exactly once on every normal completion of the function: the path from
  the call to the body crosses only positions evaluated once and
  unconditionally, and no earlier statement of an enclosing block can leave by
  `return`, `break` or `continue`. `async` functions and generators state
  nothing. Anything else keeps `min: 0`. This is a proposal input only.
- **The certifier proves `min: 1` from the producer, not from the proposal.**
  An owner-requirement `create`, `cleanup` or `compute` with a per-call count
  of `1..many` is certified by the operation's own evidence under the strict
  floor its `min: 1` sets; after ADR 0159 that admits only a dialect primitive
  call of the export's own frame the producer states `unconditional`. The upper
  bound stays `many`. Every other tight count is refused as before.
- **A return site's reach is read at its lower bound.** The `return` arm of
  `require_operation_evidence` filtered return sites on the producer's
  optimistic `reach == reachable`; it now reads `carryReach` against the
  operation's own floor, so a `return` inside an `if` no longer witnesses an
  operation that claims the return happens.

## Consequences

- `package-computation-consumer` pins the consumer arm: `startTicker()` at
  module scope (`min: 0`) moved from violation to uncertifiable, and a new
  `startTickerAlways()` (`min: 1`) is the violation. The contract corpus moved
  only owner-requirement counts from `min: 0` to `min: 1`, each on an export
  whose body calls the primitive unconditionally (listed in the commit).
- `unconditional_calls`' tests pin the syntax half on the real bytes of
  `@solid-primitives/gestures@3.0.0-next.3` `tap` (an unconditional
  `onCleanup`, so `min: 1`) against `createMutationObserver` (`if
  (isSupported)`, so `min: 0`). `owner_operation_call_follows_the_demanded_lower_bound`
  pins the census half: a reachable call under a condition refuses `min: 1`,
  an unconditional one witnesses it.
- Measured on the generator and census, release binary, head probe, host
  free, over the 46 corpus packages the misuse ledger's `missing-owner` cases
  use: 30 exports certify an owner requirement. 13 certify `min: 1` -- the
  seven `@solid-primitives/gestures` directives (`tap`, `doubleTap`,
  `longPress`, `pan`, `pinch`, `rotate`, `swipe`), `utils` `createMicrotask`,
  `marker` `createMarker`, and four `websocket` exports -- and 17 stay
  `min: 0`, each on a guarded or branching body (`timer` `createTimer`,
  `createPolled`, `createTimeoutLoop`, `devices` `createDevices`, the five
  `sensors` exports, and others). No owner-requirement operation was withheld
  for its count.
- `make primitives-checkpoint` at the ADR 0159 base and here, all three hosts:
  the headline is unchanged (721 exports, 96 clean), no export changes
  bucket, and the only cause moves are the refusal *key* an export reports
  first among several withheld operations (for example `callable-path` and
  `recursive-value-shape` trading places) -- 28, 25 and 34 exports in the
  free, browser and node hosts. No owner, `creates` or `computations` cause
  moved.
- The misuse ledger (`fixtures/primitives-misuse/cases.json`) gains a finding
  `kind`, `violation` unless stated. Fourteen `missing-owner` cases whose
  export registers only under a condition now expect `uncertifiable`, and a
  violation there is an overclaim (`misuse overclaims`); `gestures`
  `tap` is added as the unconditional counterpart, expected a violation. `tsc`
  is silent on both of its files against the published typings.
- **The accepted tier predates this, and that is visible.** Every accepted
  main was generated with `min: 0`, so every tier-derived `SC4001` is now
  uncertifiable until the tier is regenerated. In the misuse run, three cases
  that reported the violation before report it uncertifiable:
  `createMicrotask` and `createMarker`, which certify `min: 1` above and
  return to the violation with a regenerated tier, and `memo`
  `createPureReaction`, whose body starts `if (isServer) return` and which was
  the overclaim this decision removes. `createMicrotask` was criterion 3's one
  reporting case, so criterion 3 reads one lower until the tier moves.

## Remaining

- **Host constants are not folded.** `isServer` is a host constant of
  `@solidjs/web`; under a certified browser host it is `false`, and the
  `if (isServer) return;` guard never fires, so those exports do register on
  every browser call. The generator does not fold it, so they stay `min: 0`
  and uncertifiable at an unowned call in every host.
- **A disjunction is not a lower bound per kind.** `createTimer` registers an
  `onCleanup` when `delay` is a number and a `createEffect` otherwise: every
  non-server call registers something, but neither kind has `min >= 1`, and the
  vocabulary has no "at least one of" count. Such a call stays uncertifiable.
- **Depth.** A requirement reached through another contract, or through a
  local helper the export calls, is never guaranteed: the syntax walk and the
  census both look only at the export's own frame.
- **Audited rows.** A hand-audited bundle row with `min: 1` would be a
  violation by the same projection, but no solid-v2 bundled row states an
  owner requirement today, so that path has no instance.
