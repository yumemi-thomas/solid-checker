# Ways to improve the package contracts, evaluated

- Written 2026-09-24 against HEAD 0ec743c4, which re-pinned the census after
  ADR 0116 while this was being written. Read-only: no census ran, nothing was
  rebuilt, and the worktree was not touched. Every number is marked **[M]**
  (measured, with the file or run it came from) or **[E]** (estimated).
- The data: the census pin `benchmarks/ecosystem/coverage-census.json` (0ec743c4;
  run finished 2026-09-24 03:13:24Z, its `rust/target/coverage-census/run.json`
  copied before reading), the per-export evidence
  `2026-09-24-what-holds-an-import-open-after-0116.json` (and the after-0115
  file it supersedes), the retained certification outputs of that run under
  `$TMPDIR/solid-checker-ecosystem-out-*` (audits, proposals, refusals), the
  pinned `benchmarks/ecosystem/report.json` (2026-09-14), the compiled-in tier
  `pkg/contracts/accepted/` (47566ff8), the three Solid 2 demand sweeps
  `2026-09-17-demand-*.json`, and the Solid 2 artifact bytes installed under
  `$TMPDIR/solid-checker-ecosystem-c3IsJY/node_modules/` (`@kobalte/utils`
  2.0.0-alpha.0, `@solid-primitives/utils` 7.0.0-next.4). The retained trees
  under `rust/target/coverage-census/solid-checker-ecosystem-*` hold Solid 1
  versions (0.9.2, 6.4.1) and were not used.
- Terms are CONTEXT.md's: claim domain, domain closure, implementation census,
  census disposition, withheld closure candidate, parameter-rooted accessor,
  built-in runtime model, finding kind (violation / uncertifiable).
- Seven read-only explorers took one direction each; an adversarial reviewer
  then broke two of the draft's load-bearing claims (`callHandler` and the
  erasure premise, § 3.5), and this version records the corrections.

## 1. Verdict

Neither goal is reachable by adding one reviewed premise per invoking form, but
the shortfall is far more concentrated than the refusal-class totals suggest.
Of the 733 demanded sites still open, 157 are one export
(`@solid-primitives/utils` `access`) and 160 are two (`@kobalte/utils`
`callHandler`, `composeEventHandlers`) **[M, after-0116]**; the census's 261
clean callable sites are eleven exports **[M, run.json joined to demand]**.
Three directions were checked and should not be pursued now: components
through compiler lowering (0 sites; `@kobalte/core`'s default artifact is
compiled DOM code, not JSX), a browser-environment premise (0 to 2 sites), and
holding package bytes to the project-code bar as such (the generator *is* the
project summary plus a census; the sound version of the idea is "describe
instead of deny"). Composition-first is already on in the census, and the
`unaccepted-external-dependency` label on 113 domain-sites is a stale reading
of the plain lane. The plan that moves the most per unit of effort is: fix
three measurement defects so progress is countable, re-key recipes so every
later step stops costing a second census run, then land one described
`callbacks` item (an accessor read or coercion of a caller's argument, with
tracking stated as `ambient-at-execution`), which takes clean callables from
261 to about 421 **[E]**. `callHandler` needs four pieces, not the three the
draft claimed, and is the next campaign after that. For goal 2 the honest
position is that no contract claim can feed a violation rule until the
producer distinguishes a wrapper that clears tracking from one that merely
does not establish it; the item above is compatible with that fix, and
`SC9005`'s severity should be split by gate rather than lowered.

## 2. Where things stand, re-derived

| what an import finds open (1,152 in-surface sites) | pin 710403a6 | pin 0ec743c4 |
| --- | ---: | ---: |
| nothing, a non-callable value | 158 | 158 |
| nothing, a callable | 212 | **261** |
| `returns` or `callbacks` open: SC9005 on some uses | 308 | 259 |
| `reads` or `creates` open: SC9005 at every import | 474 | 474 |

Both columns are `coverage-census.json` `totals.consumer` **[M]**. ADR 0116
moved `accessWith` (27) and 16f33ed4 moved `keys` (22); `access` has `returns`
closed and waits on `callbacks`; `entries` keeps `callbacks` open on the
getters `Object.entries` runs (c7473268) and now has two domains open.

**The 733 open sites are 73 exports, and most have three or four domains open**
**[M, after-0116]**:

| open domains on the export | exports | sites |
| ---: | ---: | ---: |
| 1 | 4 | 166 (`access` 157, `compare` 3, `createIdGenerator` 3, `afterPaint` 3) |
| 2 | 14 | 108 |
| 3 | 27 | 339 |
| 4 | 28 | 120 |

A site is clean only when every domain a consumer reads is closed, so a lever
that closes one domain moves a *site* only on the four one-domain exports.
Every one of the 60 every-import exports has `creates` or `reads` open *and*
`returns` or `callbacks` open; `creates` alone is open on 472 of the 474
every-import sites **[M]**. This is why refusal-class totals ("`callbacks`
census-refused 511") overstate what any one fix buys.

**By package** **[M, after-0116]**: `@solid-primitives/utils` 463 sites,
`@kobalte/utils` 176 (170 every-import), `@solid-primitives/rootless` 39,
`@kobalte/core` 16, all others 39 together.

**Domain-sites by status** (a site counted once per open domain) **[M,
after-0116 `bySiteClass`]**: `callbacks` census-refused 511; `returns` never
proposed 287; `creates` census-refused 257; `creates` never proposed 137;
`returns` veto did not complete 112 (§ 3.5 shows this is not what it looks
like); `callbacks` never proposed 104; `reads` census-refused 80; `returns`
operation census-refused 67; `unaccepted-external-dependency` 183 (68
callbacks, 71 returns, 25 creates, 19 reads); `creates` dialect-silent 33;
`reads` no recipe 25.

**The tier users run** is 47566ff8: 1,166 export cases, 0 callables closing
all four consumer-read domains, 74 distinct (package, export) pairs carrying a
positive claim a rule consumes **[M, pkg/contracts/accepted/objects, reproducing
the assessed doc's 13/34/19/26]**. Twenty commits since have not reached it.

**The pace** **[M, git]**: ADRs 0100 to 0116 landed between 2026-09-13 and
2026-09-24; 0113 moved 161 sites, 0115 51, 0116 27, the alias amendments 59 and
22; each change to what `@solid-primitives/utils` certifies orphaned the ten
`rootless`/`trigger` recipes and cost a second census run (three times,
2026-09-23 to 24).

## 3. The directions, ranked

Ranked by expected sites moved to clean, or misuse findings enabled, per unit
of effort. "Needs" says whether the work is a model change, a producer protocol
change, a census premise, an audit reading, or a policy. § 4 gives the
sequence.

### 3.1 Fix three measurement defects, then add two metrics

**(a) The open-import script takes a withheld reason from any artifact case.**
`status()` in `2026-09-23-what-holds-an-import-open.mjs:167` does
`withheld.find(entry => cases.has(entry.artifactCase)) ?? withheld[0]`. For
`@kobalte/utils` the row has eight artifact cases, two for `.` (the dist a
consumer imports) and six for the `./src/*.ts` wildcard, which the consumer
view rightly skips (`nameableEntrypoint`, `contract-coverage-census.mjs:246`)
**[M, run.json]**. `callHandler`'s "veto did not complete" and its `creates`
"census refused" come from the `.ts` cases. In the dist cases `returns` is a
*withheld operation* ("primitive returns census refuses … whose completion the
producer did not prove primitive": `event?.defaultPrevented` is `any` in
JavaScript) and `creates` is never proposed **[M, out-SohIzt kobalte utils
audit and proposal]**. The 112-site "erasure" row in the after-0116 table is
therefore mislabelled, and § 3.5 is deferred because of it.

**(b) The script reads the plain lane's refusals.** The `declined:
unaccepted-external-dependency` label on `rootless`, `trigger`, `memo` and
`storage` (113 domain-sites, 53 sites, 9 exports **[M]**) comes from
`.refusals.json`, the plain-lane sidecar (`…open.mjs:181-189`), while those
rows ran `lane: published-graph` behind a certified utils node (7 canonical
nodes; 35 utils exports closed through the composed node) **[M, run.json]**.
The graph lane closed `reads` on `createHydratableSingletonRoot`,
`createSingletonRoot` and `createSharedRoot`, and proposed no `callbacks` or
`returns` candidate for them at all (no withheld record either) **[M]**. So
their real status is "never proposed on the graph lane", and their `creates`
is § 3.6's dialect-callee wall.

**(c) The Solid 2 demand collector keeps one message.** The sweep behind
`2026-09-17-demand-*.json` records only findings matching "has no
entrypoint/export summary" (`phase21/2026-09-12-consumer-demand-measurement.py:31-33,
122`) **[M]**. Open-claims findings never become demand rows, so
`kobalte@solid2`'s 97 open-claims findings yielded 9 demand sites. The Solid 2
demand as recorded is roughly the set of imports the tier does *not* reach,
and it shrinks as supply grows; a denominator built from it would flatter the
checker. The Solid 2 roots are also mostly the packages' own monorepos
importing workspace copies, which can never match a bundle's integrity **[E]**.

**Metrics.** Keep the census's per-export consumer view as the CI gate with the
frozen 1.x denominator (ADR 0110 § 5's reporting rule). Add a direction-gated
number that does not depend on demand: the share of each package's whole
nameable export surface that is clean or a value, computable from the census
catalogs (the tier reproduces 1,166 export cases; `run.json` has
`certifiedClosures.closed` per export but no shape). Add a "misuse-capable"
count: exports carrying an owner requirement, a `read` of an argument, a
returned accessor, or an `invoke` with a tracking statement; today 74 pairs,
31 of them in the 1.x demand (440 sites), 5 in the Solid 2 demand (15 sites)
**[M]**. Do not gate on a per-project grain: nothing on disk carries per-site
context (recensus rows are `{sites, projects, package, export, closed, open,
state}` **[M]**).

**Moves**: 0 sites; it decides what counts and it already misranked one
direction. **Needs**: scaffold. **Soundness cost**: none. **Effort**: 2 to 3
days; a further day plus network for a corrected Solid 2 sweep (10 to 25 min
wall, 1 to 3 GB of installs **[E]**). **Proved wrong by**: a corrected
collector finding Solid 2 demand almost entirely workspace-internal, which
would make delivery, not coverage, the wall.

### 3.2 Durability: re-key recipes off dependency digests

A recipe is addressed by `claimId` alone (`runtime_probes.rs:337`), and the
claim id hashes the artifact case, whose `dependency_closure` digest hashes
each dependency's `accepted_contract_digest` (`canonical.rs:88-101, 235-246`;
`artifact_resolution.rs:1293-1313`) **[M]**. The claim's *value* is not in the
key. So every change to what utils certifies orphans every dependent's
recipes; 710403a6 and 0ec743c4 each record the 36-site degenerate regression
and the verbatim carry-over **[M]**.

**Change**: a second recipe address hashing package and export identity, claim
path, a byte-only case identity (entrypoint, trace, artifacts, closure entries
with each dependency's own byte-only identity, recursively) and the claim's
normalized value digest; dropping `accepted_contract_digest`. About 57 of the
141 unique no-recipe claims match an existing recipe stem by (export, domain)
under another case **[E, name match]**.
**Soundness cost**: none if the value digest is included (same bytes, same
claim). Without it a recipe could go silent against a broadened claim, and a
silent recipe *satisfies* the gate.
**Moves**: 0 sites directly; removes one census run and a carry-over from
every later ADR (§ 3.3, § 3.4, § 3.6 each pay it otherwise).
**Effort**: about a week; 366 entries, the addressing script, scaffold and
corpus test **[E]**.
**Synthesized vetoes**: nothing new is sound. Every candidate shape from ADR
0036 to 0116 has a synthesizer; the "no recipe" withholdings are 141 unique
claims, 337 row-records of them empty `reads: []`, whose contradiction (a read
of a source the export owns) no module can observe by design
(`synthesized_vetoes.rs:568-575`) **[M]**. Of the 25 demanded no-recipe sites,
0 would close: each has another domain open **[M]**.

### 3.3 Describe instead of deny: an invocation item for a caller's accessor or coercion

The semantic model already says a getter reached by property access, an
iteration-protocol member, a coercion and `Symbol.hasInstance` are invocations
of caller-supplied code (`semantic-model.md` § callbacks), and the producer
already records every such form with `subjectParameter`, `captured`, depth and
location (`rust/crates/typefacts/src/invocation.rs:982-1108`) **[M]**. The
certifier throws that away: `CallerSuppliedInvocations::record` counts per
member only, and ADR 0100 rule 2 refuses on any non-direct member
(`contract_certification/type_facts.rs:10266-10269, 10988-10999`) **[M]**.
Nothing has to be discovered; a positive item has to be expressible.

**Item A: an accessor read or coercion of a parameter.** "Reads a property of
your argument (or converts it) at call time, on your stack, in your tracking
context; any trap of your value may run." The join over the `callbacks`
refusals naming only accessor / iterable / coercion / has-instance members
beside described direct calls **[M, after-0116 detail strings; bytes from
c3IsJY]**:

| export | sites | family in the refusal | other open domains | moves to clean? |
| --- | ---: | --- | --- | --- |
| `access` (`typeof v === "function" && !v.length ? v() : v`) | 157 | direct 1 (described), accessor 1 (`v.length`) | none | **yes, 157** |
| `compare` | 3 | coercion 2 (`a < b`) | none | yes, 3 (only if the item covers coercion) |
| `isPointInPolygon` | 6 | accessor 3 | `returns` | no |
| `ofClass` | 3 | accessor 1, has-instance 1 | `returns` | no |
| `get` | 3 | accessor 1 | `returns`, `creates` | no |
| `getScrollParent` | 2 | accessor 1 | `reads`, `returns` | no |
| `wrapSetter` | 3 | direct 2, accessor 3, iterable 1 | `returns` | no |

Seven exports, 177 sites close `callbacks`; **160 sites move to clean** (261 to
about 421) **[M join]**. `typeof` and `!` are not invoking forms
(`uncensused_invoking_forms.go:157, 259-268`) **[M]**, so `v.length` is the
whole of `access`'s blocker.

**Item B: a call of a member of a parameter** (the callee is `handler[0]`,
`list.map`, or an element a `for…of` yields). This is the member-rooted callee
ADR 0100 refuses by name and the `list.map(...)` member call it left open; it
is one item, not two, and item A is its prerequisite because the member read
is itself an accessor of the caller's value. Its cost is measured on
`callHandler` **[M, out-SohIzt dist cases 6d69dd5a/9dd8dc77; bytes c3IsJY:3-7]**:

| `callHandler` (112 sites) needs | status in the dist case |
| --- | --- |
| item A for `handler[0]`, `handler[1]`, `event?.defaultPrevented` | `callbacks` census refused, "an unresolved callee" |
| item B for `handler[0](handler[1], event)` | same refusal; also what stops the `creates` walk from proposing |
| a `creates` proposal in the dist case | never proposed (the `.ts` cases propose it and are refused) |
| a `returns` shape for "a property of your argument" | withheld operation: completion is `any`; ADR 0113 reads the `.d.ts` `boolean` only to refuse |

So `callHandler` is four pieces (A, B, a `creates` walk that survives B, a
property-of-argument return container extending ADR 0115/0116), and
`composeEventHandlers` (48) adds a retained-callback item, a returned-function
shape and an ADR 0038 refusal at depth 1 **[M]**. The draft's "A + B +
erasure" was wrong; the erasure premise touches only the `./src/*.ts` cases no
consumer names (§ 3.5).

**Does it apply to `reads` and `creates`?** No **[M]**. ADR 0101 already
describes member invocations on a parameter as `parameter-member` reads; a
plain property read of a caller's value is the caller's side of the
authorship line. The demanded `reads` refusals are "states no reviewed
subject" on values the package built (`pick`, `update`, `split`, `concat`, …),
sized by the 2026-09-18 backlog entry as ADR 0044-family premises. The
`creates` refusals are unresolved callees (item B), dialect callees outside the
artifact (§ 3.6), `bind`, and a call through a nested callable's parameter.
The summary also carries a `deferred` callback word (`contracts.rs:251`) that
no census confirms; a deferred-invocation item is the third describable item
and the price of `createCallbackStack`, `chain`, `reverseChain`,
`composeEventHandlers` (about 80 sites, each also needing a returned-function
`returns` shape) **[E]**.

**Model change.** ADR 0004 forbids *incompatible* normalized meaning, not
additive items; ADR 0114 added `computations` under its own digest family so
existing documents hash as before. The item needs a protocol word
(`call | get | iterate | coerce | has-instance`) on the `invoke` operation,
its wire field (`WireCallback` and `WireOperation` are `deny_unknown_fields`,
so old decoders fail closed **[M, contract_document.rs:1498-1552]**), schema,
validation, canonical encoding, a generator walk fact (the generator must
derive the item itself, ADR 0006), census confirmation applying ADR 0100's
rules 5 to 8 per protocol, a veto observation (a Proxy slot with
`get`/`has`/`ownKeys` and `Symbol.iterator`/`toPrimitive`/`hasInstance` traps,
merged with the `DescribedReads` tripwire), and the consumer projection
(`contracts.rs:189` drops `path` today). No handshake change for item A; item
B needs a new census disposition and a producer change **[E]**.

**Would it feed misuse findings?** Not directly, and it must not claim to.
`contracts.rs:220` maps `tracking: untracked` to `clears_tracking`, and
`inferred_contract.rs:793` writes `untracked` for every non-tracked row, so
`access`'s contract already says `untracked` for a bare call that inherits
tracking (`docs/precision-backlog.md`, 2026-09-17). The new item must carry
`ambient-at-execution`, the value the vocabulary defines and no shipped row
uses (0 of 156 operations **[M]**), or a future rule would report a false
positive on every accessor wrapper. Its later value for goal 2 is real: once
the producer states `ambient-at-execution` for inherited tracking and
`untracked` only for a proven clear, a described invocation is what lets a
rule say whose scope a callback's reads belong to.

**Soundness cost**: only depth-0, uncaptured sites may be claimed "at call
time"; a read deferred into a returned closure must refuse. Cardinality min 0,
max many. The consumer must understand the protocol word before any document
carrying it ships. **Effort**: item A about 2 weeks, one ADR, about ten files;
item B another 1 to 2 weeks **[E]**. **Proved wrong by**: the generator being
unable to derive the item without the census; the Proxy observation breaking
the samples (`typeof` on a Proxy, `.length` on a recording callable); a census
run after item A still refusing `access`.

### 3.4 Composition: what is left once the labels are right

Composition is already on. `make contract-coverage-census` passes
`--recover-entrypoints` (Makefile:391), and under ADR 0071 recovery prepares
the dependency frontier with the retained cases; `rootless`, `trigger`, `memo`
and `storage` ran `lane: published-graph` **[M, run.json]**. The residual
classes **[M]**:

| class | demanded sites | blocker |
| --- | ---: | --- |
| stale label (§ 3.1 b) | 53 | `callbacks`/`returns` never proposed on the graph lane; `creates` is the dialect-callee wall below |
| dependency not in the corpus | 16 (`@kobalte/core` Select, Tabs, Collapsible, Dialog, Popover) | 14 packages outside the corpus (`@floating-ui/dom`, `@internationalized/number`, twelve `@solid-primitives/*`); also `dialect-silent` `omit`/`merge`/`createSignal` and `runtime-accessor-installation` |
| dependency not installed | 2 (`keyed` `keyArray`) | utils is a `devDependency` of keyed yet imported at runtime; not soundly closable |
| core-runtime re-export refusal | 0 | binds no demanded export |

**A dialect primitive as a callee does not bind in the `creates` census.**
`createHydratableSingletonRoot` (27), `createSingletonRoot` (7),
`createMicrotask` (20), `debounce` (4) and `throttle` (1) are refused
"a resolved callee that is neither a default-library member, a dialect
primitive under the negative authority, nor a declaration in this artifact's
own runtime source" for `getOwner` and `onCleanup` declared in
`@solidjs/signals/dist/types/…` (`type_facts.rs:13369`), on the rc.0 *and*
the rc.3 rows **[M, run.json]**, although the dialect carries audited rows for
both, bound to archive `@solidjs/signals@2.0.0-rc.3` (`solid_2.rs:560-624`).
`census_dialect_axiom_for_callee` (`type_facts.rs:6068`) returned nothing for
them; why was not established here (the callee resolves to a declaration file,
and the terminator names an archive tuple). **59 demanded `creates`
domain-sites** hang on it, and it is the cheapest unexplained refusal in the
data. The experiment is one certification of `@solid-primitives/scheduled`
with the census transcript kept, to see which arm of that function declines.

**The dialect-silent 33.** `solid-js/types/index.d.ts:6` re-declares
`createSignal`, `createMemo` and `createEffect` from `./client/hydration.js`;
the `node`/`worker`/`deno` bodies of the first two reach `ctx.serialize`, and
the 2026-09-04 decision counts that as a create, so their rows are withheld on
purpose (`solid_2.rs:310-336`) **[M]**. `getOwner` and `runWithOwner` rows do
reach through the re-export on the proposal path (they appear in no
`dialectSilentBlockers`) **[M]**. Releasing the three rows needs a
condition-aware row shape (the audit's § 7.4 options c/d), since the tier binds
the archive, not the condition, and an SSR consumer runs the server body. Only
`createHydratableSignal` (20) and `createTween` (2) decline on dialect-silent
alone: 22 `creates` domain-sites become proposable, 0 exports close fully;
corpus-wide the rows unblock 46/40/36 exports, mostly `@kobalte/core` **[M]**.
This is the row between the tier and more `missing-owner` findings (goal 2).

**`tryOnCleanup`** (37) is `isDev ? fn => getOwner() ? onCleanup(fn) : fn :
onCleanup` **[M, c3IsJY:149]**: a conditional initializer the producer has no
implementation for, `onCleanup` as a callee (above), condition-aware `isDev`,
and `returns`/`callbacks` never proposed. Four walls; no cheap path.

**Cost** **[M, run.json]**: graph-lane rows take 33 to 38 s, 30 to 34 s of it
re-running utils' vetoes inline; nothing is reused across rows. Full kobalte
composition adds about 17 nodes, +2 to 10 min **[E]**.
**Effort**: relabel, half a day; the dialect-callee experiment, a day;
condition-aware rows plus a browser reading for `createMemo`/`createEffect`,
1 to 2 weeks; kobalte's 14 dependencies, 2 to 3 days with a yield near 0 while
the components' own reads and creates refuse **[E]**.
**Soundness cost**: an audit row is a human reading bound to rc.3 by
`file_sha256`/`slice_sha256`; the createEffect withdrawal (C1) shows a guarded
reach can be missed.

### 3.5 A type-erasure premise for the veto harness (deferred)

`@kobalte/utils@2.0.0-alpha.0` publishes `"./src/*": "./src/*"` beside its
dist **[M, package.json]**, and the pinned Node 24 refuses to strip types under
`node_modules`, so 26 gate records in the `./src/*.ts` cases end "veto did not
complete" **[M, run.json]**. ADR 0039 named this case as "an erasure premise,
not this one". The shape exists (strip-only output for authenticated `.ts`
closure files in the worker's load hook, digest recorded as
`ts-strip-esm:<count>:sha256:<digest>`), it partly supersedes ADR 0009 and
inherits ADR 0030's caveat. **But no consumer imports `./src/*`**, the consumer
view skips wildcard entrypoints, and the dist cases fail elsewhere (§ 3.1 a).
**Moves**: 0 consumer sites **[M]**. Deferred until a nameable entrypoint ships
TypeScript.

### 3.6 What a consumer sees: severity by gate, `creates` at the call

**Severity.** `SC9005` is `error, uncertifiable` (`rules.rs:177`); `SC9011` and
`SC9012` are warnings, `SC9013` an error; rule options take `enabled` only, so
a native user's escape is to disable the rule and lose every fail-closed
answer **[M]**. No ADR fixes the severity; the assessed doc left it open. The
A/B on `kobalte/packages/core` showed the tier turning 45 acceptance-gate
findings into 139, 97 naming open domains **[M]**. Recommendation: keep
**error** at the acceptance gate (no accepted contract: analysis under a missing
premise, as `SC9013`) and make the open-claims gate a **warning** by default.
This *inverts* the one existing per-finding precedent, which raises an
uncertain owner requirement to `error` (`findings.rs:140-152`), and should say
so. A warning is still an explicit uncertifiable result; fail-closed is kept.

**`creates` at the call.** The only consumers of projected owner requirements
are the two `missing-owner` paths (`owners.rs:789, 1118`), and both skip a call
inside an owner-providing region **[M]**. The import-level obligation can move
to each call and be discharged when the call is root-owned or exactly `OWNED`
(no UNOWNED, LEAF or COMPONENT_UNCERTAIN bit) and not `DirectiveApply`; any
non-callee reference keeps the import obligation, as `returns_shed_symbols`
does. At most 345 sites lose the import-level obligation **[M]**, but all 345
still have `returns` open and the largest (`callHandler`) is called inside
event handlers, which are unowned; imports fully cleared are few **[E]**. Two
narrowings are unsound and were rejected: `reads` scoped to "already tracked"
(the dangerous case is the untracked position; settled in
`phase21/2026-09-10-sc9005-demand-scoping-design.md` § 13) and `returns`
widened to "never flows into a tracked scope" (a returned accessor read in
untracked rendering is `SC1001`).
**Needs**: a `ContractDefectSite::Call` variant, emission from the owner pass,
a severity override; a consumer re-run emitting per-call role and owner
context to measure any of it. **Effort**: severity about 3 days; `creates` at
the call 1 to 2 weeks **[E]**.

### 3.7 Package-specific rules

The model has owner requirements with two capabilities, operation cardinality
over `trigger`/`call`/`resource` scopes counting the *package's* operations,
argument-side guards, and nine resource kinds with no context resource
(`contract_semantics.rs:1351-1526`) **[M]**:

| rule | expressible? | proof path |
| --- | --- | --- |
| "must render under provider X" | no; a new item-only domain in its own digest family (ADR 0114 pattern), so ADR 0004 need not be amended | from the bytes in principle: `useContext` is a dialect primitive with an audited slice; a dialect-axiom disposition plus a control-flow census on the branch that throws. Blockers: no `throw` operation kind and no positive `throws` item has ever existed; every `@kobalte/core` export is declined on all four domains; the consumer side is a new JSX-ancestry rule, not `SC4001` |
| "call once" | no; a bound on the consumer's calls is a new kind of fact | not a property of the bytes; an author-declared claim needs a fifth claim-knowledge state, kept outside the receipt, never discharging anything, feeding only an uncertifiable warning |
| "stable accessor", "do not mutate" | no | as above |
| "pass a getter, not a value" | out of scope | TypeScript's job where typed (`access` takes `MaybeAccessor`); the absolute rule forbids repeating it |

**Upper bound** **[E, by export name]**: provider 18, call-once 36: about 54
of 733 open sites, about 0 of the 419 clean ones. **Effort**: provider domain
weeks and blocked by § 3.4; declared claims an ADR plus a week.

### 3.8 Automated certification of each publish

Bundles bind one exact `(name, version, integrity, entrypoint, conditions)`
(`policy2_receipt.rs:790-807`); a new version silently stops matching **[M]**.
The lines are `next.1` to `next.4` and `alpha.0`, about 1 to 3 publishes a
month **[E]**; compute per batch about 3 min. A portable receipt channel exists
(`Policy2ReceiptProvenance::{BuiltIn, PersistentLocal, Portable}`,
`policy2_receipt.rs:481-506`) but catalog entries bind importer and specifier,
not the artifact root, and there is no distribution channel **[M]**. Needs: a
registry watcher, a networked CI job over the new version *and its dependents*,
§ 3.2 first, a portable artifact-keyed catalog, an issuer key, a revocation
policy. 1 to 4 weeks plus an ADR **[E]**. Until then, regenerate the tier
(`make accepted-bundles`, which re-issues receipts without re-proving) at each
release; users run 47566ff8 today.

### 3.9 Rejected or deferred

**Hold package code to the project-code bar (ADR 0001 reopened).** The
generator already *is* the project summary: the backend builds the same IR
over the package files (`allowJs: true, checkJs: false`, `type_facts.rs:1599`)
and takes `contract_export_function`'s output; `inferred_contract.rs:680-775`
turns the summary's `returns: Known(None)` and `creates: []` into unknown
unless a walk, a reviewed row or a census vouches **[M]**. "Never proposed"
means "the summary said nothing and the generator refused to read that as a
negative". Trusting the summary's negatives on untyped bytes reads unknown as
negative in three places (silent skips at `interproc.rs:4656, 4705`; the
`returns`/`creates` defaults), safe for project code only because `tsc`
checked the source and the owner graph answers `creates`. Upper bound if done
anyway: 420 to 430 of 733 sites **[E]**, nearly all utils, none of
`callHandler`, retained callbacks, owner-registering calls or components. The
sound variant, "accept the summary's closure where the census confirms every
site", is § 3.3 stated generally. Of ADR 0001's four rejections, "human review
prevents package-scale automation" is now contradicted by the chosen method;
the other three stand. Dropping the probe gate alone moves 0 sites **[M]**.

**Components through compiler lowering.** 0 of 733 **[M]**. No demanded
export is blocked by a JSX form; `@kobalte/core`'s declines are 63,784
`unaccepted-external-dependency`, 6,481 `unresolved-callee`, 1,798
`refusing-callee-fixpoint`, 1,038 `dialect-silent`, 890
`runtime-accessor-installation`, 0 JSX **[M, report.json]**. The package
publishes a compiled default case (62 of 114 dist files call
`createComponent`/`template`/`insert` **[M]**) and a `solid` case with real
JSX (60 of 114 `.jsx` **[M]**), so lowering reaches only the `solid` case; the
default case needs `@solidjs/web`'s helpers as dialect axioms. The generator
already runs the compiler on package bytes; the missing piece is a
certifier-side bridge from a `JsxElement` form to recomputed compiler execution
facts, plus a consumer path attaching `computations` to JSX elements
(`owners.rs:786-815` attaches them to calls only). Soundness: it trusts the
fork's lowering as the package's runtime semantics, which the `solid` condition
hands to whatever transform the consumer runs. Revisit after § 3.4.

**Browser-environment premise.** 0 to 2 demanded sites **[M]**: the run has 0
`ambient-declaration` refusals; the pinned report has 33, all on
`@kobalte/utils@0.9.2` (Solid 1) and one on Solid 2 (`getDefaultLocale`, 0
sites). The 1.x DOM helpers are among the 722 absent sites. For later: ADR 0047
already admits `instanceof EventTarget` as the engine's and states the
unpatched-realm assumption, so the census holds it for calls and constructors
and refuses it only for accessors; and the premise is true only with receiver
finality (never an `Element`, whose custom-element subclass can override any
getter) and an exclusion table of host operations that run user code
synchronously (`focus()`, `blur()`, `click()`, `dispatchEvent`,
`requestSubmit()`, `reportValidity()`, `showPopover()`, custom-element
reactions on `createElement`/`appendChild`/`innerHTML`/`setAttribute`/
`cloneNode`). Hold.

## 4. Recommended sequence

1. **Fix the three measurement defects and add the two metrics** (§ 3.1). The
   smallest experiment that confirms or refutes the ranking below: re-run
   `2026-09-23-what-holds-an-import-open.mjs` with the case filter made strict
   (no `?? withheld[0]`) and reading the graph lane's refusals, over the
   retained `out-*` directories of the 0ec743c4 run, which are still on disk.
   Prediction: `callHandler`'s `returns` becomes "withheld operation" and its
   `creates` "never proposed"; the 113 `unaccepted-external-dependency`
   domain-sites become "never proposed" for `callbacks`/`returns` and the
   dialect-callee refusal for `creates`. If instead the graph lane shows
   candidates the plain lane lacks, § 3.4 is worth more than ranked.
2. **Re-key recipes** (§ 3.2).
3. **Item A** (§ 3.3), with tracking `ambient-at-execution`. Prediction:
   `access` and `compare` clean, 261 to about 421.
4. **The dialect-callee experiment** (§ 3.4): one certification of
   `@solid-primitives/scheduled` with the census transcript kept. It decides
   whether 59 `creates` domain-sites are a binding bug (days) or a row-shape
   question (weeks).
5. **Severity by gate** (§ 3.6), independently and early.
6. **The `callHandler` campaign**: item B, the `creates` walk under it, the
   property-of-argument return container. Prediction: 112 sites, about 533.
7. **Condition-aware `createSignal`/`createMemo`/`createEffect` rows** and the
   producer's `ambient-at-execution` for inherited tracking (§ 3.4, § 3.3),
   which together are the precondition for any misuse rule fed by a contract;
   then the provider domain (§ 3.7) if kobalte's dependencies have composed.

Steps 1 to 5 are about 5 to 6 weeks **[E]** and take clean callables from 261
to about 421; step 6 is another 3 to 4 weeks for 112 **[E]**. The remainder is
the `./immutable` family (values the package built), the retained-callback
shapes, and `tryOnCleanup`.

## 5. Decisions only the owner can make

1. **Which number is the goal.** Recommendation: keep the frozen 1.x
   denominator, add the whole-surface share per package, and never re-pin
   demand from the current Solid 2 sweep until its collector counts every
   import.
2. **Whether `SC9005` at the open-claims gate is an error.** Recommendation:
   warning there, error at the acceptance gate, stated as an inversion of the
   owner-requirement precedent.
3. **Whether a described invocation may state `ambient-at-execution` while the
   producer cannot yet prove a clear.** Recommendation: yes, and forbid any rule
   from reading `untracked` as cleared until the producer emits both words.
4. **Whether to reopen ADR 0001.** Recommendation: no; amend its rationale to
   record that the normal path became human work per form, and build § 3.3.
5. **Whether condition-aware dialect rows are in scope**, given that a
   consumer contract's meaning then depends on the `solid-js` condition the
   downstream bundler selects. Recommendation: yes; it is also the only path to
   more `missing-owner` findings.
6. **Whether package-specific rules are in scope.** Recommendation: the
   provider requirement only, as a proven item-only domain after kobalte
   composes; no author-declared claims until a consumer would act on one.
7. **Whether to spend on delivery** (§ 3.8) before coverage. Recommendation:
   not yet; regenerate the tier per release, and decide after a corrected
   Solid 2 sweep shows whether npm-pinned bundles reach Solid 2 consumers.

## 6. What could not be measured, and why

- **Why `census_dialect_axiom_for_callee` declines `getOwner` and `onCleanup`**
  (59 domain-sites): the run keeps no census transcript.
- **What the graph lane would propose** for `callbacks`/`returns` on the
  composed rows if asked: it proposed nothing, and the reason is not recorded.
- **Any per-site consumer context** for § 3.6; nothing on disk carries it.
- **Whether a "no recipe" candidate passes the census** once unmasked; it is
  dropped from the plan first.
- **Whether host-object refusals hide under the 26 erasure failures**; those
  cases are not consumer-facing anyway.
- **The Solid 2 demand** with a corrected collector, and its sweep cost, which
  need network. **Publish cadence** likewise.
- **Whether the project-summary variant is sound on package bytes**: run the
  IR over the utils dist and diff against the census-confirmed rows; needs a
  Cargo build.

## Reproduce

The joins are `jq` over the files named in the header; the per-export table is
`after-0116.json` `.exports[]` with `.open[].status`, and the domain-site
histogram is its `bySiteClass`. Per-case statuses come from
`$TMPDIR/solid-checker-ecosystem-out-SohIzt/@kobalte__utils@2.0.0-alpha.0--solid2--only.json.{certification-audit,proposal}.json`
and the rootless equivalents in `out-5JXTkz`, which the next census run will
replace.
