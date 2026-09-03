# closed-domain-probe-gate

The end-to-end tracer for the probe-harness binding: a proposal that closes one
claim domain, the mandatory veto that domain schedules, and hand-authored
recipes that run inside the certification transaction.

Unlike its neighbours this is **not** a generator-corpus fixture, so it is
deliberately absent from `corpus.json`. Nothing here is compared against an
`expected.json` snapshot: the generator never proposes a closed claim domain,
and the point of this fixture is what happens when a *candidate contract* does.
Its driver is `contract_certification::tests::the_probe_gate_tracer_*` in
`rust/crates/solid-facts-backend/src/contract_certification.rs`.

The closed claim under test is **hand-written in the test's proposal**, not
generator-derived. That is deliberate — the generator proposes no closure — and
it is also the fixture's main caveat: the proposal says what the declaration
says because a person made it so, and what the verifier checks is that the
producer's own enumeration agrees with it.

## What it isolates

Two pairs of exports, each pair indistinguishable to TypeScript.

| export | declared type | runtime | the claim under test |
| --- | --- | --- | --- |
| `entry` | `(() => void) \| undefined` | a callable | root `ChoiceAlternatives` closed — **certifies** |
| `driftedEntry` | `(() => void) \| undefined` | `42` | the same claim — **vetoed** |
| `run` | `(callback: () => void) => void` | creates no owner | `creates: []` — refused, no premise |
| `runCreatingOwner` | `(callback: () => void) => void` | creates an owner | `creates: []` — refused, no premise |

### The pair that certifies

`entry` and `driftedEntry` have byte-identical declared types, so their Type
Facts censuses — and therefore their closure witnesses — are identical. The
witness is what closes the domain: the producer enumerates the exported value's
two alternatives, observes both exhaustively, and the verifier requires the
proposal's enumeration to *equal* that one — the same number of alternatives,
and at every index the kind the census observed there. Nothing about a probe is
involved in that.

That per-index half is why the proposal here is `Choice([Plain, Callable])` and
not any two shapes: the census classifies an alternative only by the callability
of its root in the callable-path census, so `Plain` (observed non-callable) and
`Callable` (observed callable) are the only two proposed kinds it can decide,
and every other kind refuses with `alternative-kind premise required`. A count
comparison on its own would have accepted a proposal that named two alternatives
of the wrong kinds, because the sibling per-index `recursive-value-shape`
demands assert nothing about a structural shape's callability.

What separates the two rows is a **publisher defect**. `driftedEntry` ships a
number while its declaration promises `(() => void) | undefined`, so the
package contradicts its own declaration at runtime. `tsc` cannot see it — a
consumer reads both exports as that union, which is what the declarations say —
and a mandatory contradiction veto can. That is the whole job of a probe gate.

Two properties of the union are load-bearing, and both were read from the
producer's own census rather than guessed:

* **Neither alternative may be a string.** The producer reports a string
  alternative as locally open (`openIndex`, from `String`'s numeric index
  signature), so a union containing one can never satisfy the closure premise.
* **The alternatives have to be distinguishable shapes.** The semantic model
  refuses a choice that repeats one alternative, and two string literals are
  the same shape to it. Hence `callable | undefined`.
* **The proposal's alternative order is not the author's.**
  `normalize_knowledge` sorts every knowledge set, so the proposal's index for
  an alternative is its position in the model's canonical order, and it has to
  line up with the producer's enumeration index. `Plain` sorts before
  `Callable`, which is why the proposed union is written `[Plain, Callable]`
  and matches a census whose alternative 0 is `undefined`. A shape that sorts
  the other way round is compared against the wrong census alternative and
  **refuses** — a false refusal, never a false pass. For a `Callable`
  alternative the sibling `recursive-value-shape` demand for
  `[ChoiceAlternative(i)]` enforces the same correspondence independently, so
  this fixture is not the only thing holding it up; for a `Plain` one, the
  per-index kind comparison is. See the ADR for why a multiset comparison was
  rejected.

`Component` is *not* one of the two decidable kinds, even though
`recursive_value_callability` treats it as callable: the receipt-bound artifact
would name the alternative `component`, which claims props, an element result,
and a render-time owner that this census never observed. It refuses with
`alternative-kind premise required` like every other structural kind, and
`the_alternative_kind_premise_admits_only_what_callability_decides` pins that.

### The pair that cannot certify, and must not

`run` and `runCreatingOwner` also have byte-identical declarations, and
opposite reactive-ownership behavior. Nothing in `index.d.ts` says which
creates an owner — that is precisely the point. A `creates: []` proposal for
either of them is refused as `UnsupportedDemand` at *witness acquisition*,
before any probe runs, because the census that discharges
`DomainExhaustiveness` is a census of the declaration and the declaration is
silent.

`probe-recipes/calls-only.mjs` exists to prove no recipe can rescue that:
pointed at `runCreatingOwner`, which really does create an owner, it observes
nothing to the contrary. A clean pass there would have been closure decided by
finite non-observation. The row never reaches the gate.

## What the probe does and does not decide

* It **cannot** establish closure. The Type Facts `DomainExhaustiveness`
  witness does that, and a passing veto means only that nothing contradicted
  the claim.
* It **can** refuse a proposal the package's own runtime contradicts.
* Finite non-observation is never negative evidence. A recipe that emits no
  events at all fails `validate_events`, so silence counts only from a recipe
  that proved it ran — which is why every recipe here emits a `call`
  enter/exit pair unconditionally.

## The recipes

`probe-recipes/` holds five hand-authored, claim-addressed modules. Each imports
its package under test by bare specifier, which resolves to the **private copy**
of the artifact snapshot inside the harness's 0700 directory — never the fixture
tree on disk.

Each entry in `recipes.json` also declares how it reaches its package
(`"importKind": "esm"` here — every module in this corpus uses a static
import). That is not bookkeeping: the export conditions Node applies to an
`import` are not the ones it applies to a `require`, so the kind decides which
file a bare specifier lands on. The harness passes the requested conditions as
`--conditions=` flags, asks the pinned interpreter which conditions it actually
applies for each kind, and then requires the worker's reported
`import.meta.resolve` answer to name the exact runtime target the Type Facts
witness read — refusing with `ConditionMismatch` otherwise. Without that, a
package whose `exports` listed a conforming `module-sync` target before a
contradicting `import` one would be certified on one file and probed against
the other, which is a false *pass*. Every recipe here therefore carries
`one export-condition selection: sibling conditional targets are unprobed` in
its `coverageLimitations`: the gate speaks for the one target it resolved to,
and a `module-sync`, `require`, `browser`, or `development` branch of the same
export is a different artifact case needing its own gate.

That bare specifier is the reason the harness writes a `package.json` next to
the copied recipes. `PACKAGE_SELF_RESOLVE` runs *before* the `node_modules`
walk, so without a nearer package scope the specifier is answered by the first
`package.json` above the private directory — `<tmpdir>/package.json`, a
world-writable location on Linux — and a planted
`{"name": "closed-domain-probe-gate-package", "exports": …}` there would hand
the recipe a conforming stub and let the closure certify on a value the package
never shipped. `docs/adr/0006-probe-harness-binding.md` has the full
disposition table.

| module | package | expected outcome |
| --- | --- | --- |
| `declared-alternatives.mjs` | faithful, `entry` | no contradiction; the veto passes and the row certifies |
| `drifted-alternative.mjs` | faithful, `driftedEntry` | contradiction; the veto refuses the row |
| `calls-only.mjs` | faithful, `runCreatingOwner` | never reached: the demand is unsupported |
| `patched-primordials.mjs` | `tampering-package` | contradiction, despite the package patching the worker's realm |
| `frozen-intrinsics.mjs` | faithful, `entry` | no contradiction — *provided* the worker froze the intrinsic prototypes before importing it |

`frozen-intrinsics.mjs` is the odd one out: it reports a property of the
**worker**, not of the package. It reads `Object.isFrozen` for
`Object.prototype`, `Array.prototype`, and `Function.prototype` at module scope
— recipe-import time, the moment control first reaches package-reachable code —
and emits the gate's contradiction marker if any is thawed. So removing a
`freeze` line from `contract-probe-worker.mjs` turns
`the_probe_gate_tracer_observes_frozen_intrinsics_in_the_workers_realm` from a
certification into a veto refusal.

It exists because the `toJSON` defence has two independent halves and **each
hides the other from any attack**. With the freeze in place a package's
`Object.defineProperty(Object.prototype, "toJSON", …)` throws, so the laundering
arm never runs and no fixture can distinguish a working serializer from a broken
one; without the freeze the serializer ignores `toJSON` anyway, so the arm runs
and changes nothing. A frame has no prototype chain, so there is no path that
launders one while the freeze holds — meaning no single package can exercise
both halves, and pretending otherwise would be the vacuous test this fixture
exists to avoid. Each half is therefore pinned separately: the serializer by
`a frame is serialized without consulting toJSON or any prototype` in
`packages/cli/test/contract-workflow.test.mjs`, which installs
`Object.prototype.toJSON` in an ordinary unfrozen realm, and the freeze by this
recipe.

### `tampering-package`

A recipe imports the package into the same realm as the harness that reports the
transcript, so package top-level code runs before any event is recorded. This
package's top level replaces `structuredClone`, `JSON.stringify`,
`process.stdout.write`, and `Object.keys` — every name the report path once
reached — *and* installs `Object.prototype.toJSON` and `Array.prototype.toJSON`,
which is the attack capturing a primordial cannot answer: `JSON.stringify`
performs `Get(value, "toJSON")` on every object it visits, so a `toJSON` on the
prototype chain is handed the worker's own run frame and can return a laundered
one with the contradiction event dropped and the rest renumbered. Its runtime
also contradicts its own declaration.

The expected outcome is a refusal on the **observed contradiction**, not merely
a broken run: the whole frame is built as null-prototype records and serialized
by the harness's own serializer, which consults no `toJSON` and no prototype;
the worker freezes the intrinsic prototypes before importing the recipe; and the
frames travel on a descriptor the package cannot name. A run that only failed
would prove much less.

Two things about the laundering arms are deliberate. They are wrapped in a
`try`, which a real attacker would not write — on the current worker
`defineProperty` throws against a frozen prototype, and a package that let that
escape would refuse the gate on a failed run, which is the weaker claim. And
they are written to produce a *well-formed* frame, so the test is genuinely
sensitive: against the pre-fix worker
`the_probe_gate_tracer_refuses_a_package_that_patches_the_worker_realm` fails
with a verified batch — a clean pass of the veto — rather than a decode error.

What that means for *this* test's reach, stated plainly rather than implied:
**the two `toJSON` arms cannot execute while the freeze holds.** They are
attacks against a plausible earlier worker, and against the current one they
throw and are caught. So this test pins the four *global* replacements
(`structuredClone`, `JSON.stringify`, `process.stdout.write`, `Object.keys` —
those do execute, because `globalThis` is not frozen) and the strong outcome,
while the serializer and the freeze are each pinned by their own test instead
(see `frozen-intrinsics.mjs` above). No arm can be added that launders the frame
while the freeze holds, because a frame has no prototype chain to inherit from.

### What the freeze costs

The freeze is a **refusal direction and never a pass**. A perfectly benign
package whose top level assigns to an intrinsic prototype in strict mode —
`Object.prototype.toString = fn`, an old polyfill, a shim — throws inside the
worker, the run fails, and the gate is refused. A closure that could have
certified does not; nothing certifies that otherwise would not. Every recipe in
this corpus carries that in its `coverageLimitations`, and
`docs/adr/0006-probe-harness-binding.md` records it as a Stage 1 cost rather
than a defect to be silently absorbed.

### The rule every recipe here follows

**A recipe must never hand `session` or `harness` to the package under test.**
That transcript API is the one legitimate path by which package code could reach
the transcript — a callback, a constructor argument, a global — and nothing in
the harness detects the handover. Every module in `probe-recipes/` therefore
imports its package, calls it with plain closures, and emits from the recipe
body itself.

## Recipe addressing

A recipe corpus is a directory holding `recipes.json` plus its modules, keyed
by exact semantic claim id. Claim ids are content digests of the normalized
claim, so they are not written by hand: run certification once, and the
refusal names the claim that has no recipe —

```
probe gate sha256:… has no recipe for semantic claim claim:v1:sha256:… in the corpus
```

— then author the module and add the entry. The tracer tests derive them from
the plan's own gate schedule for exactly this reason.

The corpus is an **input, not a root of trust**. Omitting a scheduled gate
refuses it (`MissingGate`); a vacuous recipe only fails to veto, and can never
establish closure. Corpus provenance is Stage 3 of the harness-binding work.

## The `tsc` claim, verified

`consumer/` is not decoration. It uses every probed export exactly as a real
consumer would, under `strict` with `moduleResolution: nodenext`:

```sh
packages/cli/node_modules/.bin/tsc --noEmit --project \
  fixtures/package-contracts/closed-domain-probe-gate/consumer/tsconfig.json
```

Exit 0 (TypeScript 5.9.3). That is what makes the claims here claims the type
system cannot make, and it is checked rather than asserted. It is *not* wired
into `scripts/tsc-oracle.mjs`: that corpus installs the audited Solid packages
to answer "does TypeScript already report this?" about Solid typings, and has
no lane for a fixture package of this repository's own. The command above is
the whole verification, and the fixture's declarations are what it holds
constant.

## What must stay true of this fixture

* **Each pair's declarations must stay identical.** A stub that made one
  sibling looser than the other would manufacture a distinction `tsc` can see,
  and the claim would stop being this checker's to make.
* **`entry` must satisfy its declaration and `driftedEntry` must not.** If that
  inverts, the positive case starts asserting the negative one's meaning while
  both tests stay green.
* **`tampering-package`'s arms must stay live attacks.** Each one has to be a
  name the report path would reach if the design regressed, and the laundering
  arms have to keep producing a well-formed frame. An arm that no longer works
  against a plausible earlier worker makes the test pass vacuously — which is
  exactly what happened while it patched four names and left `toJSON` alone.
* **`frozen-intrinsics.mjs` must keep observing at module scope.** Moving the
  `Object.isFrozen` reads inside `runProbeSession` would still notice a thawed
  prototype, but no longer attribute it to the recipe-*import* boundary, which
  is the property the worker actually promises.
* **`run` must create no owner and `runCreatingOwner` must create one.** The
  refusal under test is about a premise that is missing, not about the
  behavior; but a fixture whose "behavioral difference" was not real would
  stop demonstrating why the premise is needed.
