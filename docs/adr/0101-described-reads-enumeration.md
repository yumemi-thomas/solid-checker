# ADR 0101: A described `reads` enumeration the census confirms

- Status: accepted and implemented (2026-09-13); written with the
  implementation
- Date: 2026-09-13
- Owners: generator (`inferred_contract.rs` `reads_enumeration_is_confirmable`),
  certifier (`type_facts.rs` `described_reads`, `member_invocation_sites`,
  `confirm_described_reads`), veto synthesis (`synthesized_vetoes.rs`
  `Observation::DescribedReads`)
- Relation: the `reads` counterpart of
  [ADR 0100](0100-described-callbacks-enumeration.md). Lifts the "empty
  enumeration only" rule the 2026-09-10 `reads` census carried
  (`phase21/2026-09-10-reads-veto-observation-design.md` § 11) for exactly the
  items the generator already derives from the export's own body. No wire
  change: every fact read here — a call's `calleeParameter`, the use census's
  `directCall`/`aliasCall` rows, `completionForm` — is on the transcript
  already. Handshake protocol stays 56.

## Context

`reads: [] closed` denies that one invocation of the export observes the
current value of a reactive source it owns, *excluding* a read a
caller-supplied callable performs and a property access whose receiver the
caller supplied (`semantic-model.md` § reads, [Decision 2026-09-10]). The
census decides the empty enumeration by dispositioning every uncensused
invoking form of the export's body.

The generator, however, describes one more thing under `reads`: the
`parameter-member` row. `function direct(props) { props.of.values() }`
publishes a `read` operation whose input is parameter 0 at path `of.values` —
the export's own act of invoking a member of a value its caller handed it,
which for a Solid store or props proxy is the read the consumer needs to know
about (`local_access.rs`'s SC9012 obligation reads exactly these rows to decide
whether a call site's argument is reactive). The row is written only for a
member call in the export's **own body**; a member invocation inside a nested
callable leaves the domain open (`contracts.rs`, `in_owner_body`), which the
`captured-parameter-member-read` fixture pins.

Every such enumeration was proposed closed and refused by the census as "a
reads closure candidate must enumerate no operation, but the proposal names
N". Measured on the 2026-09-13 pin's recipe-less `reads` frontier by running
the census under a scaffold pass: on `@solid-primitives/utils@6.4.1`'s shared
root case six of the fourteen withheld exports are this class (`accessArray`,
`arrayEquals`, `filterNonNullable`, `handleDiffArray`, `lines`, `ndjson`), on
`@floating-ui/utils@0.2.12`'s root case four of nine (`getAlignment`,
`getOppositeAlignmentPlacement`, `getOppositePlacement`, `getSide`), on
`@corvu/utils`'s `./dom` cases two of three (`contains`,
`sortByDocumentPosition`), on `@tanstack/store@0.11.1` `shallow`. These are
not false claims the census rightly refuses: the item is a member invocation
the transcript states at a location, and the claim's other half — no owned
read — is the census the empty enumeration already passes.

## Decision

**A `reads` enumeration whose every item is an unguarded, untracked `read`
of a caller parameter `at` the call event on the same stack, performed in
this export's own frame, is proposable, and the implementation census
confirms it site for site: every member invocation of a caller parameter the
transcript states must be an item, and every item must have such a site.**
Anything else in either direction refuses by name. The empty enumeration is
**not** refuted by a member invocation: a member invocation on a
caller-supplied value is on the caller's side of § reads' authorship line, so
its absence from a proposal is not a false claim. The row is description the
generator adds when it can resolve the member, and the first corpus run
measured what refuting would have cost — 144 closed rows on `some(...signals)`,
`pipe(...transformers)`, `moveItem(arr, …)` and `removeItems`, every one a
rest or array parameter the generator writes no row for. Those closures are
correct under the domain's definition and stay; the site only counts the
invocations.

### What the generator proposes

`reads_enumeration_is_confirmable` admits the domain to `propose_closures`
when every item's operation is `kind: read`, its first input
`ValueShape::Parameter`, `at: call`, `same-stack`, `untracked`, unguarded, and
not `composed_from` another export. An owned reactive read (`Loading`'s
accessor), a row composed through a call to a sibling export, or a deferred,
tracked or guarded row keeps the enumeration **partial** and yields no
candidate — a claim the census cannot confirm is not published as a closure
it must refuse, exactly as ADR 0100 decided for `callbacks`.

### What the certifier confirms

`census_reads_domain` reads the proposal through `described_reads`, which
answers `None` for the empty enumeration and the item list when every item has
the shape above, refusing before any form is read otherwise — naming the
item and the reason (an owned source, a schedule, an execution point,
tracking, a guard, a composed provenance).

The forms walk is unchanged: every uncensused invoking form must still
disposition, which is the owned side of the claim. Then
`confirm_described_reads` reads the transcript's **member-invocation sites** —
every reachable `call` whose `calleeParameter` carries a non-empty path
(`props.of.values()`), and every reachable `directCall`/`aliasCall` parameter
use with a non-empty binding path (the call of a binding an alias or a
destructuring pattern took from such a member, which the call census by
construction never states) — and refuses, in this order, on:

1. a root that does not complete plainly (`async`, generator): a read in
   such a body may happen after the export has returned;
2. a site inside a callable nested in the implementation: the generator
   leaves the domain open when it sees one, so a proposal that reached here
   with such a site is one the generator and the census disagree about;
3. a site reaching the parameter through a computed or positional segment,
   which no described path can name;
4. a site no item matches — the same parameter, the item's path a prefix of
   the invoked one (`parameter_binding_matches`, the matcher the item's own
   witness already uses): the proposal understates;
5. an item no site matches: the proposal overstates.

For the empty enumeration the forms walk alone decides, as before, and the
site records how many uncaptured member invocations the transcript states. A
bare call of the parameter itself (`cb()`, empty path) is the `callbacks`
domain's item and is not a member-invocation site at all.

What survives is exactly the enumeration, and the site is
`typefacts-implementation-census:reads:described-invocations:<sites>:items:<n>`
(`…:reads:member-invocations:<n>` for the empty enumeration). Tracking and
owner are not confirmed: a same-stack member invocation inherits both from
its caller, `untracked`/ambient is the generator's existing word for that.

### What the veto observes

`reviewed_observation("reads")` still returns `None` for the empty
enumeration, deliberately: its contradiction is a read of a source the
export owns, which no synthesized module can see (the design record § 3-§ 4),
so that closure keeps its hand recipes. A described enumeration is different
in kind — its items are member invocations of the **caller's own values**,
which a module can hand in and watch — and is observed on the footing of ADR
0100's `DescribedCallbacks`: `Observation::DescribedReads(mask)`, the
described parameter indices as bits. Every described slot, and every slot
whose sample would be an object, array or callable, is rendered as
`tripwireAt(slot)`: a callable proxy whose every string-keyed member is
another callable proxy remembering the slot. Invoking a member records;
emitting `read-operation` happens for a member of an undescribed slot at any
time up to the end of the drain, or a member of a described slot outside the
sample call. A plain property read, a bare call of the argument itself, and
the engine's own protocol members (`then`, `valueOf`, `toString`, `toJSON`,
`constructor`, symbols) record nothing; iterating or coercing a tripwire
throws and is a `sample-threw` non-observation. Which member ran is not
observed: the census confirms the paths, the module tells the slots apart.

The reads fixture's `invokesCallerMember` is the tracer, and
`a_described_reads_closure_reaches_a_receipt_through_its_mandatory_veto`
carries the closure — proposed from the generator's own `expected.json`,
confirmed, vetoed by the synthesized module, bound — to a receipt whose
document has `reads` closed and non-empty. `invokesCallerMemberLater`, the
same call inside a `queueMicrotask` callback, is left open by the generator
and proposes nothing.

## What refuses, and why each is a refusal rather than a skip

- **A read of a source the export owns** (`Loading`, `isPending`): never
  proposed. The census can confirm nothing about an owned read beyond the
  forms walk, and publishing the closure would have it refused at
  `described_reads`.
- **A row composed from a sibling export** (`wrapper(source) { return
  access(source) }` with `access`'s parameter read composed onto `wrapper`):
  never proposed. The read happens in `access`'s frame; this census reads
  `wrapper`'s own transcript and recurses into nothing. Lifting it is
  dependency composition, sized and left in `docs/precision-backlog.md`.
- **A generator row with an empty path whose site is an argument position**
  — the IR writes `parameter` at path `[]` when the export passes its
  parameter to a call whose callee has a contracted parameter read
  (`interproc.rs`, "the parameter's own value is read, not a property of
  it"). The census finds no member invocation for it and refuses at rule 5.
  Honest: the item is true and the census did not derive it; the corpus
  measures how often it occurs.
- **A member invocation the generator wrote no row for** — a rest parameter's
  `signals.some(...)`, an array parameter's `arr.splice(...)`, a destructured
  `({ onChange }) => onChange()` — is not a refusal of the empty enumeration
  (see Decision). Under a *described* enumeration the same site refuses at
  rule 4: the proposal named some member invocations and not this one.
- **A member-chain walk under the veto** (`contains`' `while (target) target =
  target.parentNode`) ran to the session budget on the first corpus run, 102
  detail rows on four cases, because every member of a tripwire was another
  tripwire. The chain is bounded at depth eight, so the walk ends and the run
  is a clean non-observation; a described invocation sits far above that depth.

## Consequences

- No protocol change. The producer already states every fact read here.
- Fixtures: `implementation-census-reads` gains `invokesCallerMember` (closes
  with one item) and `invokesCallerMemberLater` (open); the generator
  fixtures whose `reads` was proposed closed over owned or composed items
  (`composed-operation-provenance`, `composed-operation-shadowed-target`) now
  publish those enumerations partial. Every changed export was inspected.
- Corpus effect is recorded in `docs/precision-backlog.md` under this ADR's
  entry, measured on the release binary against the 2026-09-13 pin.
