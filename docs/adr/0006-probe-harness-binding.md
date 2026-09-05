# 0006 — Bind the runtime-probe harness

Status: accepted (Stage 1 of three)
Date: 2026-09-03

## Context

Every one of the 357 certified rows in the Phase 20/21 corpus is a receipt over
**open claims only**: `exportsProven` is 0 of 3410. A contract that says
"unknown" about every claim domain is a true contract and a useless one. The
value is in a *closed* claim domain — `creates: []` on a plain function, say —
because that is a statement TypeScript cannot make and a consumer's analysis
can actually use.

No closed claim domain could certify, for four independent reasons:

1. Finalization's allowed demand-family list omitted
   `ProofFamily::DomainExhaustiveness`, so every closure candidate died as
   `UnsupportedDemand` before any probe gate was reached.
2. `ProbeGateSchedule::authenticate` unconditionally returned
   `HarnessBindingRequired`, because "the harness image and Node runtime are
   not independently pinned and directly launched".
3. Nothing connected certification to probe execution at all.
   `RuntimeProbePlan` was built from a *proposal-plan document* in the audit
   path, never from a `CertificationPlan`, and probe gate ids bind
   `snapshot_root` + `demand_graph_root`, which the probe side never saw.
4. `probe_gate_root` was hardcoded to the canonical empty root, and
   `VerifiedProbeGateBatch` had no constructor.

The decision was to bind the harness rather than to weaken the veto.

Two further reasons surfaced during adversarial review of the first attempt at
this, and both changed the design rather than being papered over:

5. Admitting `DomainExhaustiveness` for *any* closure subject let the probe
   decide closure. The witness that discharges it is a census of the
   declaration, so two exports with byte-identical declarations get an
   identical witness — and for a `creates: []` claim the only remaining
   discriminator was the probe's finite non-observation. That is closure
   decided by non-observation, which CONTEXT.md forbids outright.
6. The recipe imports the analyzed package into the same realm as the harness
   that reports the transcript, and the report path reached
   `Array.prototype.push`, `structuredClone`, `JSON.stringify`, and
   `process.stdout.write` by name. Patching `structuredClone` to drop the
   contradiction event and renumber the rest produced a clean-looking frame,
   and the gate passed.

## Decision

Make the chain real end to end, under the repository's stated trust rules,
**without changing the proof-policy digest**. `ProbeRule` and
`ProbeProducerConstraints` in `certification.rs` are deliberately untouched:
editing them changes `POLICY_DIGEST` and therefore every receipt's policy
binding, which is Stage 2 work.

### What is a root of trust

Two digests, both compiled into the verifier by the build, exactly mirroring
`TypeFactsProducerPin::configured`:

| variable | meaning |
| --- | --- |
| `SOLID_CHECKER_PROBE_HARNESS_SHA256` | the harness source manifest |
| `SOLID_CHECKER_PROBE_NODE_SHA256` | sha256 of the Node executable's regular-file bytes |

`scripts/probe-harness-source-identity.mjs` defines the manifest: a closed,
sorted file list — the driver, the harness module, the worker, the CLI probe
entry, the CLI `package.json`, both checked-in lockfiles, and the identity
script itself — hashed under its own domain string in the same framing as
`scripts/typefacts-source-identity.mjs`.

The harness is interpreted, so its source *is* its executable image, and the
Rust adapter recomputes the manifest from the bytes on disk once per gate
schedule — and keeps those bytes, so the private directory every launch runs
from is populated with what was hashed rather than with a second read of the
same paths. That is stronger than the Type Facts case, where the source
manifest cannot be recovered from a compiled binary and a stamp has to carry
it. The
stamp this build writes beside the CLI
(`packages/cli/probe-harness.buildinfo`, gitignored) is a build-provenance
cross-check compared *to* the compiled-in digest — never a root of its own. A
stamp that agrees with a tampered image still refuses, because the recomputed
manifest already failed.

A build lacking either pin refuses probe authority. The consequence is
deliberate: on such a build a nonempty probe schedule cannot certify at all. It
never silently certifies an unvetoed closure.

### Which closed claim domains the family may reach

Admitting `ProofFamily::DomainExhaustiveness` is not admitting every closure.
The censuses that discharge it — the exported value's own observation, one
selected signature's parameter and result values, the callable-path census, the
control-flow census — are all censuses of the **declaration**, and
`type_facts::require_census_decides_closure` allows only the claims a
declaration decides:

| claim path | census premise |
| --- | --- |
| `Value{root: Export, path: [], domain: ChoiceAlternatives}` | the producer's own alternative enumeration, every alternative observed exhaustively, every declared callable path closed with its subtree enumerated — *and* the proposal's enumeration required to equal the producer's, alternative count and per-index kind alike |
| `Domain(GuardPartition)` (invocation census only) | a complete finite partition in a control-flow census with no unsupported branch, alongside closed parameter and result values |
| `Domain(Call(Creates))` (implementation census only, `ClosureCensus::Implementation`) | the export's own `ExportImplementationTranscript` at the `MayExecute` floor with every invoking form enumerated (handshake protocol 14), no `calls` row withheld and every unmodelled control-flow construct classified `reachability-lower-bound` rather than `flow-unaccounted` (handshake protocol 15), every call dispositioned by its callee — `unreachable`, `parameter-rooted`, `standard-library`, `dialect-axiom`, `local-recursion` — and every local declaration recursed into through the same pinned session; the proposal's item set required to be empty. `docs/adr/0008-implementation-census-for-creates.md` |

Everything else is an `UnsupportedDemand` naming the premise it lacks. Two
groups matter:

* **The other behavioral call domains** — `reads`, `writes`, `callbacks`,
  `cleanups`, `disposals`, `invalidates`, and equally `throws` and `returns`.
  These are closures the Solid packages want, and they are **blocked**, not
  deferred by preference. Each needs its own census of every invoking form that
  reaches it (`reads` additionally the proxy property-access forms; `throws` is
  not a census target under version 1 at all), and only `creates` has one
  today. Acquiring and verifying such a census is a proof-mode change, not a
  harness one — `creates` was that change (ADR 0008).
* **The other value domains** — object properties, tuple items, array bounds,
  capabilities, and any non-root path. A declaration decides these too, but
  this census hands over no enumeration of them to compare a proposal against,
  so admitting them would certify the proposal's own word.

Why the enumeration comparison is the load-bearing half: without it, a closed
claim would rest on "the declared type is finite and fully observed", which is
true of a proposal naming two alternatives and of one naming five. The census
enumerates; the proposal has to match. That is what makes the closure the
census's claim rather than the proposal's.

Comparison is by count **and** by index, and the second half is not decoration.
The sibling per-index `recursive-value-shape` demands a closed choice
inventories carry a `DemandedCallability` that is `Unknown` for every
structural kind — `Object`, `Tuple`, `Promise`, `Reactive`, `Store`, `Action`,
`Cleanup` — so a count comparison alone would have let a proposal name two
alternatives of the *wrong kinds* and still "equal" a census of two.
`require_export_alternative_kind_matches_census` therefore compares each index
against the only classification this census carries, which is the callability
of that alternative's root in the callable-path census: a proposed `Callable`
alternative must be observed callable, a `Plain` one observed non-callable, the
index must appear in the producer's own enumeration, and its root observation
must be present and locally closed. **Every other proposed kind refuses** with
`alternative-kind premise required`, because the census never described that
structure and admitting it would certify the proposal's own word. The gap that
leaves — no producer *kind* fact per alternative, so closure over a union of
structural shapes is unreachable — is recorded in
`docs/precision-backlog.md`.

`Component` is among the kinds that refuse, and that is a decision rather than
an omission. `recursive_value_callability` groups it with `Callable`, correctly
for the question that function asks — a component is callable. But this
comparison decides whether a *closed* claim may certify, and the artifact a
receipt then binds names the alternative `component`. The only evidence in hand
is bare callability, which every function has; nothing here observes a props
parameter, a JSX or element result, or a render-time owner. `component` would
therefore be a strictly stronger name than the census can support, so it gets
the same refusal every unclassifiable kind gets. A publisher who wants the
closure can propose `Callable`, which is the claim the evidence makes.

Comparing by index presumes the two enumerations are ordered alike, and neither
side chose to be: the proposal's order is `normalize_knowledge`'s canonical
sort, the producer's is its own. It is a *checked* property rather than a
coincidence, and it is checked for **both** callability kinds by the sibling
per-index demand rather than by this comparison. `inventory_value_shape`
(`solid-reactive-ir`) emits one `recursive-value-shape` demand per alternative
carrying `recursive_value_callability(item)` — `Callable` for a `Callable`
alternative, `NonCallable` for a `Plain` one — and
`require_export_recursive_subject` verifies it against the census fact whose
`alternative` field is `i` through `require_path_callability`, which refuses a
demanded `NonCallable` against an observed callable exactly as it refuses the
converse. Every scheduled demand must be discharged, so a permuted enumeration
refuses there whichever kind sits at the permuted index.

What this comparison contributes on its own is therefore not permutation
coverage but **the refusal of every kind the census cannot classify**: each
structural shape's demand carries `DemandedCallability::Unknown`, which the
sibling demand accepts without asserting anything, so a proposal naming two
alternatives of the wrong structural kinds would otherwise have "equalled" a
census of two. It also requires the index to appear in the producer's own
enumeration, and that alternative's root observation to be present and locally
closed. The multiset comparison — matching the counts of callable and
non-callable alternatives instead of their positions — was rejected on exactly
that ground: it keeps the unclassifiable-kind refusal and so buys no reachable
certification, because every permutation it would newly accept is already
refused by the sibling demand; what it costs is coherence, leaving two halves of
one premise disagreeing about what an index means. The residual cost of the
index reading itself is a false refusal, not a false pass, and it is recorded in
`docs/precision-backlog.md`.

Two facts about the producer shaped the tracer fixture, both read from its own
census rather than guessed. A string alternative is reported locally open
(`openIndex`, from `String`'s numeric index signature), so a union containing
one can never satisfy the premise. And the alternative *order* is the
producer's, not the declaration's, because the per-alternative
`recursive-value-shape` demands a closed choice inventories are looked up by
alternative index.

### Who launches what

Rust launches the worker. `contract-probe-driver.mjs` no longer decides
anything semantic inside a certification transaction; it remains the audit
path's process driver and a measurement harness.

Per gate schedule, `probe_harness::run_probe_gates`:

1. resolves the Node path, refusing a symlink or any non-regular file — a
   symlink is a name that can be repointed at other bytes after the pin was
   taken — and hashes the bytes against the pin. That hash is *one read* of a
   path that is read again to ask for `--version` and again by every `exec`, so
   the pin is not trusted once: `node-executable` is the one watched path with
   an expected value known in advance, and every census — before the first
   launch, between launches, on every exit path — compares it against the
   **pin** rather than against the baseline. What this does not cover, stated
   plainly: bytes swapped in and reverted between two censuses are not
   detected, which is the residual reason the path must be a real regular file
   rather than a name;
2. recomputes the harness manifest and cross-checks the stamp;
3. asks the *pinned* Node bytes for their own version (as trustworthy as the
   bytes, which were verified first);
4. loads the claim-addressed recipe corpus, hashing each module itself to
   derive its `construction` digest;
5. asks the pinned Node bytes which export conditions they actually apply, for
   an `import` and for a `require`, under exactly the `--conditions` flags a
   launch will carry — and refuses when its own resolution replay under that
   set selects a different runtime target than this transaction certifies (see
   "Export conditions" below);
6. derives the runtime-probe plan from the retained opaque plan and the gate
   ids — the mode matrix from the gates' artifact cases, the environment's
   recorded conditions from the measurement in step 5, the authority forced to
   `ClosureFalsification` because a gate is a veto — never from a
   caller-returned plan document;
7. creates a private 0700 directory, **refusing outright when any location
   outside it could answer a bare specifier** — every `<ancestor>/node_modules`
   and the CommonJS `$PREFIX/lib/node` — and then watching every one of those
   candidates for the rest of the transaction; writes *the bytes step 2 hashed*
   plus the recipe modules into it, alongside an `exports`-less `package.json`
   in `harness/` and `recipes/` that terminates `LOOKUP_PACKAGE_SCOPE` inside
   the tree; and materializes a **private copy of the artifact snapshot** under
   `node_modules/<package>/` — the package name validated as an npm name first,
   because the upstream coordinate check admits a `..` segment — remembering
   the artifact case's own runtime target inside that copy. It then places the
   **authenticated dependency closure** beside it, one
   `node_modules/<dependency>/` per snapshot this transaction already
   authenticated, refusing first when the analyzed package imports a dependency
   nothing authenticated or when one name has two authenticated versions. The
   census is taken
   after the permission changes and before the first launch, and re-taken
   between launches and on every exit path. "Module resolution: the disposition
   table" below enumerates each resolver step this covers, and what is left
   *not denied*;
8. launches each isolated repeat in **its own process group**, with
   `env_clear()` plus a short allowlist (`HOME`, `TMPDIR`, `LANG`, `LC_ALL`, an
   emptied `NODE_OPTIONS`, the recipe path, and a per-launch nonce), an argument
   vector of one `--conditions=` flag per requested export condition followed by
   the worker path, and a cwd inside the private directory. `PATH` is
   deliberately absent: the worker is launched by absolute path and must not
   find anything by name. The group is killed on every exit path, which is what
   bounds the transaction — a detached grandchild inheriting the report
   descriptor would otherwise hold the pipe open forever, and no reader is ever
   joined for that reason;
9. hands the worker a pipe on **descriptor 3** and reads both frames there,
   bounded in frames and bytes, refusing anything other than exactly one
   startup frame and one run frame. Stdout is `/dev/null`;
10. requires the worker's startup frame to echo the protocol, this launch's
    nonce, and the Node version, platform, and architecture the verifier
    established, killing the process group on any mismatch — requires the run's
    reported process identity to name the pid Rust actually launched — and
    requires the resolution the worker reported for the recipe's declared
    import kind to name the artifact case's own runtime target inside the
    private copy — and, for every dependency specifier the recipe declared,
    requires the reported answer to name a file *inside* that dependency's
    authenticated private copy.

The worker's runtime, isolation, and environment fields are transport data that
must **equal** what Rust computed. It echoes `session.mode.environment`
verbatim and computes no digest about itself, because a digest a process
computes about itself is worth nothing. The `os` and `architecture` it echoes
are the *verifier's* `std::env::consts`, so the startup frame carries Node's
own `process.platform` and `process.arch` and Rust compares them through a
mapping that refuses an unmapped platform rather than skipping the check.

### The worker's realm is the package's realm

The recipe imports the analyzed package, so package top-level code runs in the
worker before any event is recorded, and everything the report path reaches by
name is the package's to replace. Patching `structuredClone` alone was enough
to drop the contradiction event, renumber the rest, and pass the gate.

Three answers, all in the design rather than in a rule:

* Every primordial the report path needs is captured into module-local
  bindings while `contract-probe-harness.mjs` and `contract-probe-worker.mjs`
  evaluate — which is strictly before the recipe, and therefore the package, is
  imported. The raw descriptor write, `Buffer.from`, `Object.keys`,
  `Object.create`, `Object.getPrototypeOf`, `Promise.resolve`, `setTimeout`,
  `Error`, and `String` are all held that way. `structuredClone` is simply
  gone: an event is copied field by field into a frozen null-prototype record
  of scalars, which also means a getter, a proxy, and a later mutation cannot
  change what is reported, and a recipe cannot number its own events.
* **Capturing `JSON.stringify` was not enough, and that was a live hole.** The
  algorithm performs `Get(value, "toJSON")` on every object it visits and
  serializes whatever comes back, so the lookup is part of the algorithm rather
  than of the binding: a package top level doing
  `Object.defineProperty(Object.prototype, "toJSON", …)` was handed
  `this === frame` — the real session, environment, and isolation — and could
  return a laundered frame with the contradiction event dropped and the rest
  renumbered. `Array.prototype.toJSON` is the same attack aimed at the events
  container. A `replacer` does not fix it, because `toJSON` runs first. So the
  whole frame — the run frame, the outcome, the isolation, the echoed
  environment, and the events container — is built as null-prototype records
  and lists and serialized by the harness's own `serializeFrame`, which walks
  own keys and indices and consults no `toJSON` and no prototype. The captured
  `JSON.stringify` survives only as a *scalar* escaper: a string, a finite
  number, and a boolean are primitives, so the algorithm performs no `toJSON`
  lookup on them at all. The worker additionally freezes `Object.prototype`,
  `Array.prototype`, and `Function.prototype` before importing the recipe, so a
  package that tries the patch throws — which refuses the gate — instead of
  succeeding. **Against the `toJSON` attack, either half suffices**; both are
  present because this is the one attack that produced a clean-looking
  certification. That is the whole of the claim: the freeze is not a general
  realm sandbox (globals are still replaceable, in-realm loader hooks are not
  denied), and the frame representation defends the report path and nothing
  else.

  Neither half is load-bearing for the other, and each is now pinned by its own
  test — which took some care, because *each hides the other from any attack*.
  With the freeze in place a package's `defineProperty` throws, so a laundering
  arm never runs and no test can tell a working serializer from a broken one;
  remove the freeze and the serializer ignores `toJSON` anyway, so the arm runs
  and changes nothing. There is no path that launders a frame while the freeze
  holds — a frame has no prototype chain for an inherited `toJSON` to sit on —
  so no single fixture can exercise both, and stating that plainly is the honest
  answer rather than inventing an arm that "executes". So:
  `a frame is serialized without consulting toJSON or any prototype`
  (`packages/cli/test/contract-workflow.test.mjs`) installs
  `Object.prototype.toJSON` in an ordinary unfrozen realm and asserts
  `JSON.stringify` launders while `serializeFrame` does not; and
  `probe-recipes/frozen-intrinsics.mjs` with
  `the_probe_gate_tracer_observes_frozen_intrinsics_in_the_workers_realm`
  reports `Object.isFrozen` for all three prototypes from inside a launched
  worker at recipe-import time, emitting the gate's contradiction marker when
  any is thawed. Removing the serializer fails the first; removing a `freeze`
  line fails the second (verified by doing it: 62 passed, 1 failed).

  Nothing on the report path *depends* on the freeze, which is now also true
  rather than merely intended. The two places that did — a `for…of` over
  `session.drain`, which reads `Array.prototype[Symbol.iterator]`, and a
  `typeof controls.flush` on the prototyped `?? {}` the worker substituted for a
  recipe's missing return, which reads `Object.prototype` — are an index loop
  and an `Object.hasOwn` lookup on the value the recipe actually returned.

  The freeze has a cost, and it is a **refusal direction, never a pass**: a
  benign package whose top level does `obj.toString = fn` in strict mode throws,
  the run fails, and the gate is refused. A closure that could have certified
  does not; nothing certifies that otherwise would not. It is recorded in the
  fixture README and in every recipe's `coverageLimitations`.
* The frames travel on descriptor 3, a pipe the launcher created and handed
  over. `process.stdout.write` reaches `/dev/null` under the certification
  harness and a drained pipe under the audit driver, and neither is read.

`fixtures/package-contracts/closed-domain-probe-gate/tampering-package` pins
this: a package whose top level replaces every one of those names *and* installs
both laundering arms, and whose runtime contradicts its own declaration, is
still vetoed on the observed contradiction. The arms are attempted inside a
`try` — a real attacker would not catch, and would then refuse the gate on a
failed run — precisely so the test asserts the strong outcome and stays
sensitive to the frame representation itself. Against the pre-fix worker that
test fails: the laundered frame passes the veto and `authenticate_probe_gates`
returns a verified batch.

One rule the design cannot enforce, so it is a rule for recipe authors: **a
recipe must never hand `session` or `harness` to the package under test.** The
transcript API is the one legitimate path by which package code could reach the
transcript, and a recipe that passes either into a package callback, a
constructor, or a global has given it away. Rust re-derives sequence numbers
and refuses non-scalar event fields, so the damage is bounded, but nothing
detects the handover.

### Where a verdict comes from

`ProbeGateOutcome` is crate-private with no public constructor. Outcomes are
derived only from a `RuntimeProbeEvaluation` produced in the same transaction,
through a new crate-internal channel, `ProbeTargetVerdict`:

| verdict | gate outcome |
| --- | --- |
| a run observed the contradiction | `Contradiction` |
| refused / errored / timed out / never returned | `ErrorOrTimeout` |
| complete, isolated, deterministic, scenario-satisfying, no contradiction | `NoContradictionObserved` |
| a possible-positive witness target | not a gate; never satisfies one |

The distinction that channel exists for: for a closure-falsification recipe,
the *expected marker is the contradiction*. Not seeing it in a complete run is
a clean pass of the veto, not a refusal of the run — but it is still not
evidence, so the probe **evidence material** keeps recording that observation
as `Refused`. Only the separate gate verdict records that nothing contradicted.
A recipe that emits no events at all fails `validate_events`, so silence counts
only from a recipe that proved it ran.

### What authenticates and what refuses

* An **empty** schedule authenticates on its own. The certifier walked the
  normalized artifact case and found no proposed closure; it is not going to
  launch a fake harness to say so. The receipt binds the canonical
  domain-separated `empty("empty-probe-gate-schedule")` root, byte-identical to
  before this change, so every receipt already issued stays valid.
* A **nonempty** schedule authenticates only against a
  `BoundProbeHarnessIdentity`, which exists only after the sequence above ran,
  and only for the exact snapshot root, demand-graph root, and gate ids it was
  bound to.
* The receipt's `probe_gate_root` for a nonempty batch is domain-separated over
  the policy digest, the demand-graph root, `schedule-version:1`, the sorted
  gate ids, and the harness/runtime identity: harness manifest digest, Node
  executable digest, Node version, worker protocol, sandbox policy digest,
  runtime-probe plan digest (which transitively binds each recipe's bytes and
  the environment identity, and so the measured export-condition sets), and
  recipe-corpus root (which binds each recipe's declared import kind alongside
  its file name and construction digest). Per-launch nonces, pids, and launch epochs are
  deliberately excluded — they bind the live worker inside the transaction,
  while a receipt root has to stay reproducible from the same inputs.
* A substituted Node binary, harness script, or recipe with the same version
  string is refused. So is a missing pin, a stale stamp, a symlinked Node path,
  a symlinked or escaping recipe module, and an over-large probe input.

A passing probe still never establishes closure. The Type Facts
`DomainExhaustiveness` witness (`type_facts::require_domain_closure`,
`require_closed_value`, `require_export_callable_paths_closed`) is what proves
a claim domain enumerates every behavior possible for that exact export. The
gate is a veto and nothing else. Non-observation is never negative evidence.

### Write isolation: detect and refuse, not deny

The policy text
(`docs/package-contract-v2/phase19/2026-08-29-proof-policy-v2-and-refusal-reduction-plan.md`
lines 382-392) requires the probe to run "in a sandbox that denies writes to
snapshot and producer/compiler inputs" or refuse.

**Stage 1 does not deny writes.** It implements the property that requirement
protects — that no probe run can have altered the inputs the rest of the
transaction reads — verifiably, and refuses when it cannot show that:

* the probe reads a private copy of the artifact snapshot inside the 0700
  directory, never the shared materialized store and never the analyzed tree.
  The **authenticated dependency closure** is copied beside it, under the same
  discipline — authenticated snapshot bytes, an `npm`-validated name as its
  directory, its own watched census entry — so a recipe can `import` the
  package under test and that package can resolve its own dependencies. A
  dependency the package imports and this transaction authenticated no snapshot
  for refuses the gate by name rather than the probe reaching bytes this
  transaction never authenticated;
* the private directory itself is censused **non-recursively** — its direct
  entry names and kinds — because `TMPDIR` and the cwd are that directory, so a
  `node.config.json` or any other loader-affecting file landing beside the three
  subdirectories has to be a change. Its cost is a refusal direction: a probe
  that writes a temporary file into `TMPDIR` refuses the gate;
* digests of the whole private `node_modules` tree (not only the copied
  packages: a planted sibling would otherwise answer a bare specifier
  unwatched), **each authenticated dependency copy under its own
  `private-dependency:<name>` label** — redundant with the whole-tree digest on
  purpose, so a dependency file changing mid-run refuses by name rather than as
  an anonymous change somewhere under `node_modules`, and so a future change
  that stopped placing a copy inside the watched tree fails loudly — the copied
  harness image, the copied recipe modules — the two
  private package scopes among them, since they live in those trees, and with
  them `<private>/harness/node_modules` and `<private>/recipes/node_modules`,
  the two first rungs of the `node_modules` walk — the two
  CommonJS "global folders" a `HOME` inside the private directory makes
  resolvable (`<private>/.node_modules`, `<private>/.node_libraries`, both
  recorded as absent), `$PREFIX/lib/node` (the third global folder, computed
  from the pinned Node executable's install prefix rather than from the
  environment), every `<ancestor>/node_modules` and every `<ancestor>/package.json`
  up to the filesystem root (the `node_modules` ones recorded as absent because
  the precondition below just proved them so; a `package.json` that exists is
  recorded by content, and is unreachable anyway because the private scopes
  shadow it), the Node executable, the Type Facts producer image, and the
  verifier's own image are recorded before the first launch;
* the census is keyed by label, and a duplicate label refuses rather than being
  merged. This is a **defensive invariant, not a fixed live bug**: nothing in
  `create` builds two entries under one name today. It is enforced because the
  census is consulted by label — the pinned Node digest is looked *up* — so a
  future entry that collided would leave one path unreadable and the other
  silently authoritative, and `verify_unchanged`'s comparison would agree in
  length while disagreeing about which path each entry is.
  `a_repeated_watched_label_refuses_rather_than_being_merged` constructs the
  collision, because an invariant nothing can reach is an invariant nothing
  tests;
* the Node executable is compared against the **pin** on every census, not
  against the baseline: two censuses of a binary that was already substituted
  agree with each other, so self-consistency is not the property wanted for the
  one watched path whose bytes this build knows in advance;
* they are re-hashed **between** launches as well as after the last one —
  launches are sequential, so without that one session could tamper with what
  the next reads and restore it before the final census — and the census runs
  on every exit path, a launch failure or timeout included, with an isolation
  violation taking precedence over the launch error;
* any change — a new file in a watched tree, a watched file replaced by a
  symlink, or a watched path that was absent and now exists — refuses the gate
  with a typed `IsolationViolation`.

Resolution is contained as well as watched, and the enumeration below is the
form that took. Three review rounds each found a *different* escape from the
private workspace — an ancestor `node_modules`, the `HOME`-relative CommonJS
global folders, and an ancestor `package.json` self-reference — and each was
found by walking one more step of Node's resolver by hand. Patching one step at
a time was not converging, so the algorithm is enumerated instead.

### Module resolution: the disposition table

Every step of Node's ESM and CommonJS resolvers, the input each consults, and
this workspace's disposition. Four dispositions:

* **CONTAINED** — the step can only resolve inside the private tree, and the
  mechanism is named.
* **WATCHED** — outside the private tree, in the watched census, recorded with
  the absent marker unless noted.
* **REFUSED** — a precondition; the gate refuses before a launch happens.
* **NOT DENIED** — a documented Stage 2 limit. **This is the default for
  anything that cannot be contained or watched**, and it is stated rather than
  omitted.

`sandbox_policy_digest` (`probe_harness.rs`) mirrors these field names into the
receipt-visible policy digest, now at `scheme-version:10` (ADR 0030).
Version 7 also binds the canonical package-name/snapshot-root materialization
manifest into each nonempty probe root. Graph-authenticated dependencies and
compiler-source snapshots enter the same collision checks, private copies and
watched census; no resolver escape or isolation guarantee is relaxed.
Version 8 additionally binds the explicit inert ESM execution profile, its
pinned strip-only interpretation, watched derived bytes and exact-source-URL
hook. This profile has a separate controlled consumer and receipt version 3;
ordinary published-byte certification does not enable it.
Version 9 extends that hook to the parser-proved import-free grammar, binds
recipe replay by the controlled consumer, and advances the separate receipt to
version 4 and the worker protocol to v4. Ordinary certification still never
installs the hook or accepts the controlled receipt.
Version 10 adds ADR 0030's authenticated relative TypeScript graph. Every
source and derived output is bound and watched, and the worker resolves only an
exact `(importer URL, literal specifier, target URL)` edge map produced by the
native authenticated snapshot resolver. Controlled receipt version 5 and worker
protocol v5 carry the graph; ordinary consumers continue to refuse it.
ADR 0033 adds a *sibling* scheme rather than a version 11 of this one: the
browser profile's policy vector (`scheme-version:11`,
`scheme-family:browser-cdp-pipe`) describes a boundary with no Node resolver,
a request-stage exact URL map and a named unwatched profile-directory
carve-out, so it is digested separately and this vector is byte-identical.
Controlled receipt version 6 carries both families; ordinary consumers refuse
both.

#### ESM (`ESM_RESOLVE`)

| step | input it consults | disposition |
| --- | --- | --- |
| dependency materialization before resolution | transaction-authenticated source snapshots and transitive graph dependency snapshots | **CONTAINED / REFUSED / WATCHED** — exact snapshot bytes only, equal roots deduplicated, conflicting package roots refused, every copied dependency watched; scheme 7 binds the materialization manifest into the probe root |
| specifier is a valid URL: `file:`, `data:` | the URL itself | **NOT DENIED** — bypasses package resolution entirely; a `file:` URL reads anything the launching user can read, and a `data:` URL needs no filesystem at all |
| specifier is a valid `http(s):` URL | the network | **NOT DENIED** in the policy sense, though this Node build has no network-import support, so it fails at resolution. Network access itself is not denied |
| specifier starts with `/`, `./`, `..` | the importer's URL | **NOT DENIED** — a relative path can climb out of the private tree |
| specifier starts with `#` → `PACKAGE_IMPORTS_RESOLVE` | `LOOKUP_PACKAGE_SCOPE(parent)` → `imports` | **CONTAINED** — the private package scope has no `imports`, so a `#specifier` fails with `ERR_PACKAGE_IMPORT_NOT_DEFINED` instead of reaching an ancestor's map |
| `PACKAGE_RESOLVE`: builtin (`node:*`, bare builtin) | the builtin table | **NOT DENIED**, deliberately: the worker itself imports `node:crypto`, `node:fs`, `node:url` |
| `PACKAGE_RESOLVE` → `PACKAGE_SELF_RESOLVE` → `LOOKUP_PACKAGE_SCOPE` | the first `package.json` at or above the importer | **CONTAINED** — `<private>/harness/package.json` and `<private>/recipes/package.json` end the climb, and carry no `exports`, so self-resolution returns undefined. **WATCHED** as well: every `<ancestor>/package.json` is censused, so planting one mid-run refuses and a future change that drops a private scope fails loudly |
| `LOOKUP_PACKAGE_SCOPE` from inside the snapshot copy, or inside a dependency copy | `<private>/node_modules/<pkg>/package.json`, `<private>/node_modules/<dependency>/package.json` | **CONTAINED** — the climb returns null at a `node_modules` path segment, so it cannot pass the private `node_modules`, and the manifest it finds first is authenticated snapshot bytes. A dependency copy carries its **own** authenticated `package.json` and this module writes none there: that manifest's `exports` map is what answers its specifiers, and replacing it with the `exports`-less private scope would break the resolution the closure exists to permit. The two private scopes in `harness/` and `recipes/` are therefore unreachable from a dependency's own files, which is correct — they exist to terminate the climb for the *importers*, not for the packages |
| `PACKAGE_RESOLVE`: the `node_modules` walk | `<dir>/node_modules` for the importer and every ancestor | **CONTAINED** to `<private>/node_modules` (a watched tree, whole, not only the copied packages). The walk now has more rungs, because package code is a real importer: from `<private>/node_modules/<pkg>/…` the first candidate is `<private>/node_modules/<pkg>/node_modules`, and from a dependency copy `<private>/node_modules/<dependency>/node_modules`. Neither exists unless the *snapshot itself* ships one — in which case those are authenticated bytes too, part of the snapshot, so a nested copy shadowing a hoisted one is not an escape — and both sit inside the watched `private-node-modules` tree, so one appearing mid-run refuses. Above them the walk reaches `<private>/node_modules` and stops mattering: two candidates come before it for the *harness* importers and are named here because they are easy to miss — the importer's own directory is `<private>/harness/` for the worker and `<private>/recipes/` for a recipe, so `<private>/harness/node_modules` and `<private>/recipes/node_modules` are the first two rungs there. Neither exists, and both sit inside a watched tree — `hash_tree` covers every regular file under `harness/` and `recipes/` — so one appearing mid-run changes that tree's digest and refuses. Every ancestor candidate *above* the private directory is **REFUSED** as a precondition and **WATCHED** afterwards, because the precondition is point-in-time on a tree this process does not own (`/tmp` is world-writable on Linux) |
| `PACKAGE_RESOLVE`: a bare specifier naming a **dependency** of the analyzed package | `<private>/node_modules` | **CONTAINED** to the *authenticated* dependency closure, and **WATCHED** per copy. `PrivateProbeWorkspace::create` copies every dependency snapshot this certification transaction already authenticated — `plan.certification_sources`, the integrity-verified published archives whose lock selection replayed, the same set `type_facts::snapshot_source_roots` materializes the private Type Facts project from — into `<private>/node_modules/<name>/`, `@scope/` kept, each name through the same `safe_package_directory` validation as the analyzed package, each tree hashed under its own `private-dependency:<name>` label. Nothing is read from the project's real `node_modules`, from a registry, or from any path outside that set. A dependency the closure replay recorded an accepted edge for and this transaction authenticated **no** snapshot for is a typed refusal naming the specifier, the package, and the importer (`ProbeHarnessError::UnauthenticatedDependency`), before the private directory exists. **One version per name**: two authenticated snapshots of one name at different snapshot roots refuse (`AmbiguousDependencyVersion`) rather than one being chosen between — see "Why one version per name" below. See also "What the closure does not answer" |
| `PACKAGE_EXPORTS_RESOLVE` / `PACKAGE_TARGET_RESOLVE` | the resolved package's own `exports` | **CONTAINED** — read from the private copy's manifest; Node refuses a target that escapes its package directory |
| `LOAD_PACKAGE_EXPORTS` legacy `main` | the resolved package's `main` | **CONTAINED** — same manifest, same directory |
| `ESM_FILE_FORMAT` / package.json `"type"` | the nearest package scope | **CONTAINED** — the private scopes declare `"type": "module"`, so a file's format is decided inside the tree rather than by an ancestor |
| `ESM_FILE_FORMAT`: automatic module-syntax detection (Node ≥ 22.7) | the file's own bytes | **CONTAINED**, and it qualifies the row above rather than contradicting it: for an ambiguous `.js` with no nearer scope Node may decide the format by *parsing*, so `resolution:file-format-from-private-package-scope` means the scope that decides is inside the tree, not that a scope is the only input. Both inputs are authenticated — the private scope this module writes, and the snapshot bytes themselves — so neither reaches outside |
| `import.meta.resolve` | the same algorithm | **CONTAINED** — it inherits every row above; it resolves and does not load |
| symlink realpathing (no `--preserve-symlinks`) | the filesystem | **WATCHED** — `hash_tree` refuses any non-regular entry in a watched tree, so a watched file replaced by a link to elsewhere refuses |

#### CommonJS (`require`)

| step | input it consults | disposition |
| --- | --- | --- |
| relative / absolute request | the filesystem | **NOT DENIED**, as for ESM |
| self-reference (`trySelf` → `readPackageScope`) | the first `package.json` at or above the parent | **CONTAINED** — the same private scope, the same missing `exports`. Verified against a real interpreter, not by reading the spec |
| `Module._nodeModulePaths` walk | `<dir>/node_modules` upwards | **CONTAINED** / **REFUSED** / **WATCHED**, as for ESM |
| `NODE_PATH` | the environment | **REFUSED** — `env_clear()`, and `NODE_PATH` is not in the allowlist |
| `$HOME/.node_modules`, `$HOME/.node_libraries` | `HOME` | **CONTAINED** — `HOME` is the private directory. **WATCHED**, absent |
| `$PREFIX/lib/node` | `dirname(dirname(execPath))`, *not* the environment | **REFUSED** as a precondition and **WATCHED** absent. `env_clear` cannot remove this one: it is derived from the pinned Node executable's own install path. A Node installation that carries this legacy directory refuses probe authority, deliberately |
| `main` and `index.js` fallback | the resolved package's directory | **CONTAINED** |
| `require.extensions`, `Module._resolveFilename` patching | the worker's own realm | **NOT DENIED** — see "The worker's realm is the package's realm"; the report path reaches nothing by name, but resolution is not defended in-realm |

#### Loader-affecting inputs

| step | input it consults | disposition |
| --- | --- | --- |
| ADR 0026/0028/0030 controlled ESM profiles | native complete syntax whitelist, authenticated source graph, native expected outputs, native replayed relative edges and compiled Node/harness pins | **CONTAINED / REFUSED / WATCHED** — the trusted worker alone installs strip-only format overrides for exact selected `.ts` URLs, after primordial capture/freeze and before recipe import. Node output must equal every native expected output and watched derived file. The inert profile calls its sole export directly; the import-free and relative-graph profiles replay one selected recipe. The relative profile accepts only its receipt-bound exact edge map. Unmapped imports, URL variants and CommonJS refuse. Every frame echoes the binding and confirms the root source was loaded and the controlled consumer completed. No hook is installed for ordinary certification; controlled receipt v5 cannot enter the policy-2 consumer |
| ADR 0033 controlled browser profile (`chromium-headless-shell-cdp-pipe-esm-v1`) | compiled browser bundle pin, pinned-Node reproduction of every derived module, the checker's exact URL map, a Rust-authored page bootstrap | **CONTAINED / REFUSED / CARVE-OUT** — a separate scheme (`scheme-version:11`, family `browser-cdp-pipe`, `BROWSER_SANDBOX_POLICY_FIELDS`), not a change to this table's Node scheme. The browser has no package resolver: every request is intercepted at request stage and answered from authenticated derived bytes or fails *and refuses the launch*, and the requested URL set must equal the served map. The browser's own `--user-data-dir` is a named subtree inside the private directory whose contents are deliberately unwatched. Workers, iframes, service workers, downloads and network are refused by CSP, auto-attach and flags the pinned browser enforces — denial by the instrument, stated as such. `probe_harness/browser.rs` carries the browser's own disposition table |
| `NODE_OPTIONS` (`--import`, `--require`, `--loader`, `--experimental-loader`, `--conditions`, `--preserve-symlinks`) | the environment | **REFUSED** — `env_clear()` plus an explicitly emptied `NODE_OPTIONS`. This one matters most: `--import` runs a module *before* the worker evaluates, which is before its primordials are captured and before the intrinsic prototypes are frozen |
| command-line flags | `argv` | **CONTAINED** — Rust builds the whole argument vector: one `--conditions=<name>` flag per requested export condition (each validated as a plain condition name first), then the worker path. Nothing else, and nothing from the environment |
| `--env-file`, `--env-file-if-exists` | `argv`, then the named file | **REFUSED** — by argv, not by the environment: these are flags, so `env_clear()` does not bear on them, and the vector above contains no flag but `--conditions`. `NODE_OPTIONS` cannot smuggle one either (it is emptied, and Node disallows `--env-file` there) |
| single-executable applications, `--build-snapshot`, `--snapshot-blob` | `argv`, then a blob or the executable's own trailing resource | **CONTAINED by the byte pin alone** — no such flag is passed, and a SEA's embedded main would be *part of the executable*, so it is covered by `SOLID_CHECKER_PROBE_NODE_SHA256` and by the census that re-asserts that digest on every pass. Stated because it is the one loader input no argv or environment rule would catch: the pin is the whole of the answer |
| every other `NODE_*` / arbitrary variable (`NODE_COMPILE_CACHE`, `NODE_REPL_EXTERNAL_MODULE`, …) | the environment | **REFUSED** — the allowlist is `HOME`, `TMPDIR`, `LANG`, `LC_ALL`, an emptied `NODE_OPTIONS`, `SOLID_CHECKER_PROBE_RECIPE`, and `SOLID_CHECKER_PROBE_NONCE`. `PATH` is deliberately absent |
| `node.config.json` (`--experimental-config-file`, `--experimental-default-config-file`) | the cwd | **CONTAINED** *and* **WATCHED** — no such flag is passed, and the cwd is the private directory; but the cwd and `TMPDIR` are both that directory, so it is also censused **non-recursively** (`private-directory-entries`, direct entry names and kinds) and a `node.config.json` appearing beside the three subdirectories refuses. The row no longer rests on the absent flag alone. Its cost is a refusal direction: a probe that writes a temporary file into `TMPDIR` changes that entry census and refuses the gate |
| resolution `conditions` | the requested set via `--conditions`, plus the interpreter's own defaults | **CONTAINED**, but by two mechanisms rather than by a constant, and this row was wrong before this change — see "Export conditions" below. Rust passes one `--conditions=` flag per requested condition, then *asks the pinned bytes* which conditions they actually apply for each import kind and records that in the environment identity the worker echoes. What makes the row sound is neither of those: it is that the worker reports `import.meta.resolve(<specifier>)` (and the `createRequire` resolution) before importing the recipe, and Rust **REFUSES** unless the reported target is the exact runtime target the Type Facts witness read |
| `module.register`, `module.registerHooks` | the worker's own realm | **NOT DENIED** — a recipe or a package top level can install a resolve hook. It gains no filesystem reach a `file:` import does not already have, and reads are not denied either. It cannot forge the resolution echo either, which is read before the recipe is imported |
| native addons (`process.dlopen`), descriptor-inheriting children | the worker's own realm and its open descriptors | **NOT DENIED** — descriptor 3 is nameable in-realm (`fs.writeSync(3, …)`), a native addon runs outside every JavaScript guarantee, and a child process inherits the descriptor unless the spawner closes it. What refuses a forged frame is not secrecy of the number: a frame has to carry this launch's nonce and name the pid Rust launched, and a *third* frame on the descriptor is a protocol refusal rather than a choice of which one to believe. So an in-realm writer can spoil a run — a refusal — but cannot substitute a laundered one |
| `--permission`, policy manifests | not passed | **NOT DENIED** — OS- and runtime-level denial is Stage 2's whole subject |

Three rows deserve their reason spelled out, because they are the ones that were
wrong before this change.

### Export conditions: measured, flagged, and checked against the artifact case

The **`conditions`** row said `["import", "node"]` was CONTAINED because
"`--conditions` cannot reach it". Every clause of that was wrong, and the
failure direction was a false *pass*. Measured on the pinned Node 24.11.1, with
the launch's own environment allowlist:

| package `exports` | `import()` loads | `createRequire` loads |
| --- | --- | --- |
| `{module-sync, import, require, default}` | `module-sync` | `module-sync` |
| `{import, require, default}` | `import` | `require` |
| `{development, default}`, request asks for `["import","development"]` | `default` (the "production" target) | `default` |

So the applied set is not `["import","node"]` — this interpreter also applies
`module-sync` and `node-addons`, applies `require` rather than `import` to a
`createRequire`, and applies a *requested* condition not at all unless the
launch passes the flag. Meanwhile `plan.import_request.export_conditions`
selects the artifact case the gate subject names and the Type Facts witness
reads. A package with a conforming `module-sync` target beside a contradicting
`import` one was therefore certified on the `import` target and probed against
the `module-sync` one: nothing contradicted, and the closure certified under a
recorded condition set that was a fiction.

Three mechanisms, and only the third is load-bearing:

1. **The recipe declares its import kind** (`importKind: "esm" | "require"` in
   `recipes.json`, required and not defaulted), because the two kinds resolve
   under different sets. Rust binds the declared kind into the plan — it is
   part of the recipe-corpus root — and checks the resolution *that kind*
   produced.
2. **The requested conditions are passed as `--conditions=` flags**, each
   validated as a plain condition name first, so the interpreter applies the
   set the artifact case was selected under wherever it can. This is *not*
   sufficient on its own and is not treated as such: row 1 above is measured
   *with* the flags, and `module-sync` still wins.
3. **The worker reports what it resolved, and Rust requires the artifact
   case's own runtime target.** Before importing the recipe — and therefore
   before any package code runs — the worker resolves the plan's specifier
   both ways and puts both answers in the run frame;
   `verify_reported_resolution` refuses with `ConditionMismatch` unless the
   answer for the declared kind names the exact file the witness read. A
   missing report is a refusal, not an absence.

   The ESM answer comes from the worker's own module URL
   (`<private>/harness/`) rather than the recipe's (`<private>/recipes/`),
   because `import.meta.resolve` is per-module and Node exposes no
   resolve-from-another-URL API. The two directories carry byte-identical
   package scopes and share one `node_modules` ancestry, so they can only
   disagree if a `node_modules` appears inside one of them — and both trees are
   watched whole, so that is an `IsolationViolation` on the next census. The
   CommonJS answer is resolved from the recipe's URL, which is exact.

There is a **planning-time** refusal as well, before anything is copied or
launched: Rust replays its *own* resolver under the condition set the
interpreter reported, and refuses when that selects a different runtime target
than the transaction certifies. This is a reproducibility check rather than a
subset test — a `require` condition requested for an ESM recipe refuses here
whenever the package's `exports` would answer an `import` key first — and it
does not subsume mechanism 3, which is what proves the interpreter itself
landed where Rust's replay said it would.

What the record now carries: `EnvironmentIdentity::conditions` holds tagged
entries rather than bare names — `requested:<c>` for what the artifact case was
selected under, and `esm:<c>` / `require:<c>` for what the pinned bytes
reported that they apply — so the two facts cannot be conflated again. The
candidate list the observation asks about is a constant
(`OBSERVED_CONDITION_CANDIDATES`), which bounds what the record *names*; a
condition Node applies that is absent from it is missing from the record and
still cannot cause a false pass, because mechanism 3 refuses whenever the
selected file differs. `probe_harness::tests::the_recorded_conditions_come_from_the_pinned_interpreter`
and `::the_pinned_interpreter_can_select_a_target_the_artifact_case_did_not`
reproduce the table above against a real interpreter and assert both the
refusal and the matching pass.

### The self-reference row

The **self-reference** row was a false *pass*, not a refusal, which is the
worse failure direction. `PACKAGE_SELF_RESOLVE` runs before the `node_modules`
walk, so a `<tmpdir>/package.json` naming the analyzed package with an `exports`
map answered the recipe's own bare import: the probe observed a conforming stub,
nothing contradicted the proposal, and the closure certified. The fix is one
file per importing directory — a `package.json` with no `exports`, `imports`, or
`main` — which ends `LOOKUP_PACKAGE_SCOPE` inside the 0700 tree while being
unable to answer anything itself.
`probe_harness::tests::an_ancestor_package_self_reference_cannot_answer_a_private_bare_specifier`
plants that ancestor, runs a real interpreter, and asserts the private copy wins
for the ESM import, the `#` specifier, and the CommonJS `require` — and then
removes the private scope and asserts the ancestor *does* win, so the test
cannot go vacuous.

### The `$PREFIX/lib/node` row

The **`$PREFIX/lib/node`** row is the one CommonJS global folder `env_clear`
cannot reach, because Node computes it from `process.execPath` rather than from
`HOME`. It is refused and watched exactly like an ancestor `node_modules`.

Precondition and census are both needed throughout; neither replaces the other.

### The authenticated dependency closure

The dependency row above was a **REFUSED** one, and refusing was the honest
answer while nothing carried the bytes: a consumer package's veto recipe has to
`import` the package under test, that package's module top level then runs its
own `import "solid-js"`, and with only the analyzed package copied that is
`ERR_MODULE_NOT_FOUND` — a failed run and a refused gate. Every consumer
closure candidate was therefore unprobeable, which
`docs/adr/0008-implementation-census-for-creates.md` § "What this does not yet
buy on real rows" recorded as one of the three blockers to a certified closed
claim on a real row.

What changed is *which authenticated bytes the workspace carries*, and nothing
else. The closure is `plan.certification_sources`: the integrity-verified
published archives whose lock selection this transaction replayed, the exact
set `type_facts::snapshot_source_roots` materializes the private Type Facts
project from and the source census holds every producer-consulted file to.
`authenticated_dependency_closure` derives it; `copy_snapshot_into` — one
definition, shared with the analyzed package's own copy, so a dependency can
never enter under weaker rules — writes it. Four properties, each enforced in
code and pinned by a test:

* **Only authenticated bytes.** No read of the project's real `node_modules`,
  no registry, no path outside that set. A dependency the independently
  replayed module closure recorded an accepted edge for and this transaction
  authenticated no snapshot for is
  `ProbeHarnessError::UnauthenticatedDependency`, naming the specifier, the
  package, and the importer, before the private directory exists. A *recipe*
  that declares a specifier nothing authenticated refuses too, separately and
  by name, because the two are different claims.
* **A layout that resolves.** `<private>/node_modules/<name>/`, `@scope/` kept,
  the name through the same `safe_package_directory` validation the analyzed
  package's is. Each copy keeps its own authenticated `package.json`.
* **Every copied tree is watched**, whole, before the first launch, between
  launches, and on every exit path — under `private-dependency:<name>` as well
  as inside the whole-tree `private-node-modules` digest.
* **The resolution echo extends.** A recipe declares `dependencySpecifiers` in
  `recipes.json`; Rust binds them into the recipe-corpus root, asks the worker
  to resolve each one *before* the recipe (and therefore the package) is
  imported, and refuses unless every answer lands inside that dependency's
  authenticated copy — in the order asked, one per specifier, so a frame with a
  different count or a permuted order is a refusal rather than a set to search.

#### Why one version per name

Two authenticated snapshots of one name at different snapshot roots refuse
(`AmbiguousDependencyVersion`) rather than one being chosen between, and the
nesting alternative — placing the importer-specific copy at
`<private>/node_modules/<importer>/node_modules/<name>/` — was rejected rather
than deferred. The authenticated source set does carry an
`installed_package_root`, but that root describes the *project's real tree*, and
reading it as a nesting instruction would make the probe's resolution depend on
a path this workspace does not reproduce: the private tree has no project root,
no workspace layout, and no hoisting history. Choosing one copy would be
substitution, which is the failure mode
`type_facts::retain_collision_free_source_packages` already refuses on the Type
Facts side for the same reason. The corpus does not need otherwise — every
measured row's closure names each dependency once — so a row that genuinely
installs two versions of one package refuses its gate, and that refusal is
recorded rather than worked around.

#### What the closure does not answer

* **The echo is evidence about the rung, not about the package's own walk.**
  `import.meta.resolve` is per-module and Node exposes no resolve-from-another-
  URL API, so the ESM answer comes from `<private>/harness/` and the CommonJS
  answer from `<private>/recipes/` — not from inside the analyzed package's
  copy, which is where the package's own `import` actually resolves from. The
  rung that answers is the same one (`<private>/node_modules`), every ancestor
  candidate is refused as a precondition and watched afterwards, and both
  importer directories are watched whole, so a nearer `node_modules` appearing
  inside one is an `IsolationViolation` on the next census. But the echo does
  not *prove* the package resolved there; the fixture's recipe does, by calling
  an export whose body calls into the dependency copy and refusing the gate on
  any other answer.
* **The echo proves containment, not one exact file.** For the analyzed package
  the check is equality with the artifact case's own runtime target, because
  this transaction certifies that case. A dependency has no certified artifact
  case here, and its `exports` map may legitimately answer several
  entrypoints, so the check is that the answer is *inside* the authenticated
  copy.
* **A snapshot-shipped nested `node_modules` shadows a copy.** If the analyzed
  package's own archive ships `node_modules/<name>/`, that copy wins the walk
  from inside the package. Those are authenticated bytes too — part of the
  snapshot — so it is not an escape, and it is watched with the rest of the
  tree; it is named here because it means "the copy this module placed is the
  one that answered" is not a property the workspace guarantees.
* **A dependency's own dependencies are only as complete as the closure
  supplied.** The workspace places what the transaction authenticated; it does
  not compute a transitive closure of its own. A dependency whose module top
  level imports a package nobody supplied fails at import, which refuses the
  gate.

`SandboxKind::Process` is recorded with a **verifier-computed** policy digest
over exactly this scheme, now at `scheme-version:10`. ADR 0028 extends the
restricted transformer and exact-URL hook to import-free runtime expressions,
adds recipe replay to the separate consumer fields, and advances the worker to
protocol v4. ADR 0030 adds native-replayed exact relative edges, binds every
module's source and output, and advances the worker to protocol v5. ADR 0021 adds
`snapshot:dependency-materialization-manifest-bound-to-probe-root`; the version
6 additions described below remain in force. Its field list mirrors the
disposition table's own names — `resolution:private-package-scope`,
`resolution:no-package-self-reference`,
`resolution:no-package-imports-escape`,
`resolution:no-ancestor-node-modules`, `resolution:no-cjs-global-folders`,
`resolution:no-node-path`, `resolution:no-environment-loader-hooks`,
`resolution:file-format-from-private-package-scope`, version 5's
`resolution:requested-conditions-passed-as-interpreter-flags`,
`resolution:conditions-observed-from-pinned-interpreter`,
`resolution:declared-import-kind-per-recipe`,
`resolution:artifact-case-runtime-target-reproduced-or-refused`,
`argv:worker-path-plus-requested-conditions-only`,
`report:resolution-echoed-and-compared-to-artifact-case`,
`private-directory-entries` inside the `watched:` list, and, new at version 6,
the dependency closure's own — one renamed field, five added, and one more
entry inside the `watched:` list —
`snapshot:analyzed-package-plus-authenticated-dependency-closure` (which
*replaces* version 5's `snapshot:analyzed-package-only`),
`snapshot:dependency-closure-from-transaction-authenticated-snapshots-only`,
`snapshot:one-version-per-dependency-name-or-refuse`,
`snapshot:unauthenticated-package-dependency-refuses-by-name`,
`resolution:declared-dependency-specifiers-per-recipe`,
`report:declared-dependency-resolutions-echoed-and-required-inside-the-authenticated-copy`,
and `private-dependency-copies` inside the `watched:` list — and says outright
`enforcement:detect-and-refuse`, `network:not-denied`,
`filesystem-writes:not-denied`, `filesystem-reads:not-denied`,
`absolute-and-file-url-imports:not-denied`, `data-url-imports:not-denied`,
`builtin-node-modules:not-denied`, `in-realm-loader-hooks:not-denied`,
`child-processes:not-denied`, and
`native-addons-and-inherited-descriptors:not-denied`, so a receipt can never be
read as claiming OS-level isolation. A step whose disposition changes must
change a field here and bump the scheme version, and
`probe_harness::tests::the_sandbox_policy_digest_names_what_is_not_denied`
compares the whole vector against a literal copy so that dropping or renaming a
field fails a test instead of silently re-labelling every receipt's policy
binding. The certification request cannot declare a sandbox kind or policy at
all — there is no such field — so no caller can assert an isolation property the
transaction does not have.

What this is not: a write outside every watched path is not prevented; a read of
anything the launching user can read is not prevented, and neither is an import
by absolute path or `file:` URL, which bypasses `node_modules` resolution
entirely; child processes are not prevented; and **network access is not
denied**. A recipe that phones home does so.

Containment is bounded by the process group, too. Each launch is its own group
and the group is killed on every exit path, but a probe that calls `setsid()`
leaves that group: such a grandchild outlives the transaction, and anything it
does to a watched path after the last census is undetected. The practical
impact is limited because every Type Facts witness is acquired *before* any
probe runs, so the facts a row certifies on were read before such a process
existed — but it is a real hole in the "no probe run disturbed the inputs"
reading, and Stage 2's OS-level sandbox is what closes it, together with the
rest of this section.

### Recipes

Recipes are claim-addressed modules — hand-authored, or since ADR 0036
synthesized by the checker from the export's Type Facts call signature for a
candidate no hand recipe addresses (a hand recipe always wins) — and their
authors carry one obligation nothing here can check: **never hand `session` or
`harness` to the
package under test** (see "The worker's realm is the package's realm"). A corpus
is a directory holding `recipes.json` plus its modules, keyed by exact semantic
claim id.
Claim ids are content digests, so the workflow is to certify once and read the
refusal, which names the claim that has no recipe, then author the module.

The corpus is an **input, not a root of trust**. Omitting a scheduled gate
refuses it (`MissingGate`); a vacuous recipe only fails to veto, and can never
establish closure, which remains the Type Facts witness's job. Rust derives
every construction digest from the module bytes it copied, so a corpus cannot
present one identity and run another. A corpus that lives inside the analyzed
package is refused outright — a package must never supply the probe that vetoes
its own proposed closure. Corpus provenance beyond that is Stage 3.

## Consequences

* `DomainExhaustiveness` reaches finalization, and
  `Policy2FinalizationError::ProbeAuthorityRequired` is reachable and pinned by
  a test for the first time. Both halves were previously dead.
* **The recorded export conditions are measured, and the resolution is checked
  against the artifact case.** `conditions: ["import", "node"]` was a constant
  hashed into the plan digest and the probe-gate root while nothing enforced
  it, and the pinned interpreter applies a different set — `module-sync` among
  it, which wins over `import` for both import kinds. A recipe now declares its
  `importKind`, the requested conditions are passed as `--conditions=` flags,
  the applied sets are read back from the pinned bytes per kind and recorded
  tagged, Rust refuses at planning when its own replay under the applied set
  selects a different runtime target, and every launch's reported
  `import.meta.resolve` / `createRequire` answer has to name the exact file the
  Type Facts witness read or the gate refuses with `ConditionMismatch`.
  `the_probe_gate_tracer_refuses_when_the_interpreter_would_select_another_target`
  is the certifying row with one difference — its `exports` answers
  `module-sync` before `import`, with a contradicting runtime behind it — and it
  refuses at planning instead of certifying.
* Module resolution has a written disposition per resolver step, and the policy
  digest's `resolution:` fields mirror it at `scheme-version:6`. The private
  package scopes close the self-reference escape — a false *pass* — for ESM,
  CommonJS, and `#` specifiers at once, and `$PREFIX/lib/node` joins the
  refused-and-watched set. `an_ancestor_package_self_reference_cannot_answer_a_private_bare_specifier`
  runs a real interpreter, asserts the private copy wins, then removes the
  scope and asserts the ancestor wins, so it cannot go vacuous.
* **The dependency row is CONTAINED rather than REFUSED, and a consumer recipe
  can import the package under test.** The workspace carries the transaction's
  authenticated dependency closure beside the analyzed package's copy — only
  `plan.certification_sources` bytes, one version per name or a refusal, each
  tree watched under its own label, and a recipe's declared
  `dependencySpecifiers` echoed back and required to land inside the
  authenticated copy. A dependency the package imports and nothing
  authenticated refuses by name. `scheme-version:6` adds five fields, renames
  `snapshot:analyzed-package-only`, and adds one entry to the `watched:` list;
  no other row's truth changed
  except the `node_modules` walk, which now has the package's and each
  dependency's own first rung above it and says so.
  `a_recipe_imports_the_package_and_its_dependency_resolves_inside_the_private_copy`
  runs a real interpreter, asserts the dependency import lands inside the copy,
  then removes the copy and asserts the package stops being importable at all,
  so it cannot go vacuous.
* The watched census is keyed by label and refuses a duplicate, so
  `verify_pinned` can no longer be answered by whichever of two same-named
  entries came first. Nothing constructs such a collision today; it is a
  defensive invariant, and it now has a test that builds one.
* The private directory itself joins the census, non-recursively, so the
  `node.config.json` disposition no longer rests on the absent flag alone.
* The policy digest's field vector is asserted against a literal copy, so a
  dropped or renamed field — or a `scheme-version` bump with nothing behind it
  — fails a test instead of quietly re-labelling every receipt's policy
  binding.
* Neither half of the `toJSON` defence is untested any more, and nothing on the
  report path depends on the freeze: the drain loop is index-based and the
  flush control is an own-property lookup on what the recipe returned rather
  than on a substituted `{}`.
* `ValueShape::Component` no longer certifies from bare callability. The
  receipt-bound artifact names it `component`, which is a stronger claim than
  the census makes, so it refuses like every other unclassifiable kind.
* The audit driver launches its worker under the certification path's
  environment allowlist instead of inheriting `process.env` — an inherited
  `NODE_OPTIONS=--import …` ran a module before the worker captured its
  primordials. Its header now says outright that it establishes no realm
  integrity.
* `make test-probe-harness` exists for the fast loop, `make test-rust` sets
  `SOLID_CHECKER_EXPECT_PROBE_PINS=1` when `PROBE_NODE` resolves, and
  `verify-delta` maps the harness image's own paths to a `make` target rather
  than a `cargo` command — a bare `cargo test` there compiles a binary with no
  pins and every probe assertion returns early.
* `fixtures/package-contracts/closed-domain-probe-gate` certifies a closed
  root choice-alternatives domain end to end with a nonempty `probe_gate_root`,
  and its receipt authenticates. Its sibling export — byte-identical declared
  type, so an identical closure witness, but a runtime that ships a value the
  declaration excludes — is refused by the veto, which is the publisher defect
  a veto exists for.
* The same fixture pinned, at this ADR's cut, what a veto may *not* rescue: a
  `creates: []` proposal with a recipe that observes nothing to the contrary
  refused as `UnsupportedDemand` at witness acquisition, before a probe was
  launched, because no census decided the domain. Since ADR 0008 the
  implementation census decides it — `run` closes `creates: []` (its one call is
  parameter-rooted) and the veto then runs; `runCreatingOwner` refuses on the
  `tryReachability` marker its `try … finally` leaves, an over-refusal ADR 0008
  records — and the property this bullet protects is unchanged: the probe still
  decides nothing, and a candidate no census proves never reaches it.
* Its `consumer/` keeps the `tsc` claim verified rather than asserted: every
  probed export is used under `strict` with `moduleResolution: nodenext`, and
  `tsc --noEmit` reports nothing.
* `fixtures/package-contracts/implementation-census-creates/dependency-consumer`
  is where the dependency closure is proven end to end: a consumer whose module
  top level imports `solid-js`, its own authenticated stub, and a recipe that
  imports the package, calls the census-proved export, and calls a second
  export whose body calls into the stub — refusing the gate on any answer but
  the stub's own, so resolving is not mistaken for running. Its negative arm is
  the same package, the same recipe, the same accepted dependency edge, and no
  authenticated snapshot: the gate refuses by name.
* No existing receipt, `semanticDigest`, or fixture snapshot moves, and the
  `scheme-version:6` bump cannot move one either. Every current row has an
  empty probe schedule and keeps the byte-identical empty authority root, and
  no row in the corpus has a recipe, so none gains closure; the recipe-corpus
  root is byte-identical for a recipe that declares no `dependencySpecifiers`,
  and the sandbox policy digest only reaches a *nonempty* batch's
  `probe_gate_root`.
* `certify_value_only`, `certify_value_only_case_set`, and the published-graph
  lane all take an optional `ProbeHarnessConfiguration`, and each plan or graph
  node derives, runs, and authenticates its own veto set. A parent never
  inherits a child's probe authority.
* Probe execution happens only inside the certification transaction. Analysis,
  proposal generation, and catalog discovery never launch one.
* Type Facts witnesses are acquired *before* probes run, so a row whose
  premises do not hold refuses without executing package code.
* The audit path's environment equality check moved into
  `contract-probe-driver.mjs`, which now refuses a request declaring a Node
  version the worker is not running. Previously the worker overrode those
  fields; it no longer asserts anything about itself.

## Deferred

**Stage 2 — OS-level sandbox hardening and the policy text.** Replace
detect-and-refuse with real denial, and only then amend `ProbeRule` /
`ProbeProducerConstraints`. That amendment changes `POLICY_DIGEST` and every
receipt's policy binding, so it is an atomic cut of its own.

**The implementation-census premise for behavioral call domains.** The closures
the Solid packages want — `creates`, `reads`, `writes`, `callbacks`, and the
rest — are blocked on a Type Facts premise, not on the harness: a complete
`ExportImplementationTranscript`, every `calls` target resolved, and no
resolved target able to perform the domain's operation. Until that exists,
`require_census_decides_closure` refuses them by name, and no recipe can
substitute. This is a proof-mode change and belongs to its own slice.

Its first slice is done and it was a *semantic* one, because the domains did
not yet mean one thing: the generator emitted a resourceless `create` per owner
requirement while the hand audits used `creates` for exactly three operations,
each of which registers a resource with an environment outside the call, and
building a census over an unfixed meaning would have certified an unfixed
claim. `creates` is now defined over the **published `create` operation** —
registering a version-1 resource into a runtime outside this invocation, so it
stays live there after the call returns
(`render`'s `register-delegation` → `browser-root`,
`createServerReference`'s `register-reference` / `transform-reference` →
`server-reference`) — and explicitly *not* over the intuitive "brings something
into existence", which the audits refute: `createSignal`, `createStore`,
`createMemo`, `action`, `createEffect`, `createTrackedEffect` and `onSettled`
all establish version-1 resources and all close `creates: []`. A summary's
`resources` declaration is not a `creates` item. Owner production stays in the
`owner.productions` domain, and owner requirement stays the per-operation
`owner.requires` triple. The per-domain predicate for all nine closures is
`docs/package-contract-v2/semantic-model.md` § "What a closed call domain
denies"; the decision, the census predicate, the measured cost, and the
ordered prerequisite chain — producer enumeration guarantee, integrity-bound
dialect negative tables, then `creates`, then `reads` — are
`docs/package-contract-v2/phase21/2026-09-03-implementation-census-plan.md`.

That document also records why `complete` cannot be read as an enumeration
guarantee today, why `throws` is a model question rather than a census one, and
that version 1 has **no** domain in which a reactive-graph resource's coming
into existence is a positive closable fact — a model gap the corrected
predicate exposes rather than introduces, tracked in
`docs/precision-backlog.md`.

**The correction this ADR owed itself is taken** (ADR 0008). Under the settled
predicate the `closed-domain-probe-gate` reading is decided rather than open:
neither `run` nor `runCreatingOwner` performs a `create` operation — an object
literal in a private module variable is not a resource registered with any
runtime, and the only call in either is a parameter-rooted callee. `run` now
**certifies** `creates: []` through the implementation census, with the veto
running afterwards; `runCreatingOwner` refuses, not for what it does but because
its `try … finally` puts a control-flow marker on a transcript the census may
take no relaxation on (ADR 0008 item 0). The fixture's comments and README row
say so, and the
sibling that calls a real dialect primitive is `primitive-consumer/`'s
`runAfterSettle`, which the census refuses by name. The `creates` census is
also **recipe-gated**: a candidate whose claim has no recipe in the supplied
corpus is withheld by name before any demand or gate is derived, so returning
`creates` candidates to every consumer proposal refused no row. The remaining
behavioral call domains are still deferred exactly as the preceding paragraph
says.

**A wider value-closure premise.** Object properties, tuple items, array
bounds, capabilities, and non-root value paths need the producer's enumeration
*at that path* to compare a proposal against. Only the exported value's root
alternative enumeration is available today.

**Stage 3 — a recipe corpus for real packages.** Recipe-corpus provenance,
addressing that survives contract edits, and enough recipes for the closed
claim domains the corpus actually wants. The workspace half of this is now
done — a consumer recipe can import the package under test and that package can
resolve its authenticated dependencies — so what remains here is provenance and
the recipes themselves, plus the two blockers the closure does not touch: the
missing Solid 2.0 negative rows that keep the generator from proposing a
candidate at all, and multi-version dependency layouts, which refuse.

**Probe evidence sidecars.** The receipt binds *what was run* — gate ids,
harness identity, recipe bytes through the plan digest — but the detailed
`ProbeClaimMaterial` and transcripts are not persisted. Carrying them into an
evidence sidecar re-derives `sidecars` digests in the canonical main, which
would move existing receipts, so it needs a coordinated change.
