# Package contracts

Milestone 5 introduces `solid-reactivity.json`, a non-executable summary that
preserves exported reactive reads when a dependency's implementation source is
not part of the consuming TypeScript project.

## Workflow

The package-level generator is the default workflow for libraries with export
subpaths:

```sh
solid-checker contract generate --package-root package
```

It derives package identity from `package.json`, walks the complete `exports`
map, expands wildcard subpaths, analyzes every supported ESM implementation
target that is present in the checkout independently, follows local `export *`
barrels, checks published modules through TypeScript's resolver, and
conservatively merges conditional builds into v1 `entrypoints`. Callability,
value-space references, resolved-call validity, selected declaration ownership,
argument-to-parameter mapping, and alias identity come directly from TypeScript
compiler facts. Runtime callback timing is applied only after an exact valid
signature has been selected; same-named user methods do not inherit
standard-library behavior. **Generation never imports or executes package
code**; the one command that does is
[`contract probe`](#probing-a-generated-contract), which is opt-in and separate
for exactly that reason. From the package directory the default output is
`solid-reactivity.json`.

When conditional builds invoke the same callback parameter with different
timing, every observed timing mode is preserved. Consumers analyze each mode;
the merge does not discard one branch or treat the modes as contradictory.
Evidence-only differences do not create runtime variants: for example, equal
development and production semantics collapse to one summary whose probe modes
are unioned when both claims were independently probed. Evidence from a
narrower branch never promotes an inferred broader claim. A more-specific equal branch such as
`browser,development,import` also collapses into its broader
`browser,import` branch, preventing two variants from matching the same runtime.
`default` is the export map's unconditional branch. A generated contract
records it as the literal condition `default`, and a consumer treats it as
satisfied by any selected environment -- never by the absence of one, where
choosing a branch would still be a guess. Which branch wins among several that
match is `precedence`'s job; a handwritten contract carrying no `precedence`
resolves only the one case that needs no invented order, an explicitly named
branch beating the unconditional fallback.

A contract generated with `--conditions` is scoped to that selection. The flag
is an assertion about the resolving environment, not an observation of the
export map: it suppresses every branch the selection does not take, so the
resulting entrypoint records the selected conditions and a consumer in a
different environment fails closed instead of applying it. An entrypoint with a
single unconditional target records no conditions at all.

An entrypoint's `conditions` are therefore read as a *union of the branches it
resolves through*, not as one environment's requirement set: the bundled
`solid-js` root entrypoint records `browser, deno, development, import, node,
worker` for an export map no single environment satisfies at once. Requiring
containment would make it unmatchable, so a consumer matches on membership,
and variant selection — which does require all of a variant's conditions — is
what narrows an export afterwards. The host target (`browser`, `node`, `deno`,
`worker`) is the exception, because at most one of them describes any
environment: an entrypoint that names host targets and not the consumer's was
either scoped away from that environment or reaches it only through a branch
the contract does not describe, so the consumer fails closed instead of
matching through a shared resolver condition such as `import`. Recording
`default` keeps the entrypoint open, since the export map's unconditional
branch really is reachable everywhere.

An export name observed in only some of an entrypoint's conditional branches is
absent from the others. Schema v1 cannot say "not exported here", so generation
keeps the branches it was proven for as `variants` even when they all agree,
and normalization does not collapse a variant set that fails to cover the
entrypoint's conditions. A consumer in an uncovered environment then fails
closed rather than inheriting another branch's semantics.

The environment-unaware base a merge produces states only what *every* branch
proves. For the single-valued domains — `returns` and `asyncBehavior` — a
disagreement between branches writes the unknown sentinel into the base, and
**one-sided presence is a disagreement**: a branch that proved a `returns`
merged against a branch that proved none cannot hand the base the proving
branch's claim, because a proven summary's absence is itself a certified
negative and the base would then be false in that environment. The exact
per-branch claims survive as `variants`, so an environment-aware consumer loses
nothing. The `unknown-sentinel` review item records which branches disagreed and
how, under `because.divergences` — a merge is the second emitter of the sentinel
and used to be the silent one. Both shapes are pinned by the
`fixtures/package-contracts/conditional-returns-divergence` and
`conditional-returns-divergence-both` pair.

When targets such as browser and node change a complete export summary
(including callability or reactive behavior), schema-v1 adds
`variants`: each variant names the conditions and the complete summary proven
for that target, including its variant-local `kind`. Generated variants record
their zero-based export-map `precedence`, preserving first-match-wins fallback
ordering even when positive condition lists overlap. Handwritten variants
without one unique winning precedence remain fail-closed when several distinct
branches match. A consumer without an explicit runtime-condition selector never
guesses through a variant set or applies the merged base as a selected branch.

Bundled conformance probes each applicable claim in client, server,
development, and production condition modes, and callback probes perform both
the initial and a subsequent update. A claim that passes only in one mode is a
conformance failure: the result is a surfaced environment mismatch, not a
reason to omit that mode or silently weaken the contract. The probe runner
records successful modes and call counts as row evidence only for claims that
already exist; it never writes newly observed behavior into a contract.

Use `--entrypoint ./state` to generate only one subpath while investigating a
failure, or `--conditions browser,import` to resolve the export map for a
specific environment. With no condition selector, every materialized supported
ESM target is checked. This permits generation from a source checkout whose
export map also advertises build outputs that have not been produced yet; a
package with no materialized runtime target still fails. Compatible facts are
unioned; callability is retained only when it is valid in every selected
runtime target, and genuinely incompatible return or async summaries stop
generation.

An application developer can generate a project-owned contract without
modifying `node_modules`:

```sh
solid-checker contract generate \
  --package-root node_modules/reactive-package \
  --output .solid-checker/contracts/reactive-package/solid-reactivity.json
```

Generated TypeScript projects live in the OS temporary directory and are
removed after each entrypoint; the analyzer retains the package root separately
for export filtering and module resolution. Each project seeds the selected
entrypoint plus its exact static runtime-module closure: relative specifiers,
`#`-prefixed specifiers resolved through the manifest's `imports` map, and the
TypeScript source an ESM-spelled `./impl.js` resolves to when only `impl.ts`
exists. This keeps runtime `.js` modules ahead of adjacent declarations without
loading unrelated files from a large `dist/` tree. Package code is analyzed statically and is not
executed *by generation*. Computed module targets are not guessed. CJS-only entrypoints
currently fail as unsupported instead of receiving an inferred empty summary.

When analysis reaches an exact missing dependency boundary, generation creates
a draft for that installed dependency under the same runtime conditions, caches
it by artifact and condition set, and retries the parent. This is demand-driven:
unrelated dependencies are not scanned. Cycles and dependencies without a
supported runtime surface remain explicit failures. A recursively generated
draft is still inferred evidence; it may enumerate an export-all barrel, but
its behavioral rows do not become reviewed because a parent inherited them.

The boundary is named across the process seam by one stable line the native
checker writes to stderr beside its human message:

```
solid-checker:unresolved-dependency-module=<module specifier>
```

That line is the interface, not the sentence next to it. Recovering the
specifier from prose made a reworded diagnostic stop the recursion silently:
the parent entrypoint is then merely "refused", and a refusal exits 0. The
generator reads the marker first and keeps the older prose forms only as a
fallback for a native binary that predates it; the marker is stripped from
every message a reviewer sees, so the review plan still quotes the human
reason. The pairing is pinned end to end by
`package_generator_dependency_boundary_marker_drives_recursion` in
rust/crates/solid-facts-backend/tests/contracts_process.rs, which feeds the
real binary's real stderr to the real parser. The marker must travel *with*
the `emit package contract:` prose: a native failure without that prefix is
classified as a bug and rethrown, so the retry loop would never see it.

`contract generate` also writes two siblings of the contract: the
`<contract>.review.md` checklist a human reads, and `<contract>.review.json`,
the same plan as machine-readable items that
[`contract review`](#promoting-a-reviewed-contract) resolves one at a time.
They call out runtime entrypoints with no generated summary, the legacy manifest
field a root contract was resolved from when `module` and `main` name different
artifacts, function exports with no callback execution row, callback rows with
no owner row, generated owner requirements requiring caller review, inherited
claim rows, entrypoints whose conditional environment selection needs review,
and one generated export summary per export. The "no callback execution row"
section is load-bearing: omitting `callbacks` is a negative claim, so it is the
only place a reviewer is told which exports are about to certify "never invokes
a caller-supplied callback". The checklist is intentionally separate from
`solid-reactivity.json`: generation never promotes inferred claims or
auto-resolves the items it lists. Stdout remains one line and names every output
path — with one exception, the transfer command printed when a regeneration
snapshots a reviewed contract; see [stale contracts](#stale-contracts).

Both come from one traversal of the document that was actually written, so a
checklist line always has a machine item behind it and neither can describe a
contract the other does not. An item's `id` is derived from its kind and its
`{entrypoint, export, field}` target and never from its position, so
regenerating an unchanged package yields the same ids and every recorded review
decision stays attached to the question it answered.

Beside those exception-driven sections the plan carries one **generated export
summary** item per `(entrypoint, export)`. It is raised when the export's
summary carries any generated positive claim row — a reactive read, a return
tree, a callback row, an owner requirement, an async behavior, a variant set —
or when no other item on the plan names that export at all. Its text names the
export, lists the rows being certified, and states that the claim domains the
summary omits are certified *negative* claims. Two things follow. A contract
with exports can no longer produce an empty plan, so a package of plain values
cannot promote on zero decisions; and every export a promotion certifies is
named by at least one decision, so no row reaches `reviewed` evidence without
one. `confirm` is the only decision it takes: the item is raised for as long as
the export exists, so there is no negative to certify and no edit that answers
it.

The plan is **bound to the contract's bytes**. `contract generate` writes the
contract first, hashes it, and records that hash as the plan's `contract` field;
every `contract review` mode refuses — exit 2, nothing written — when nothing
binds the plan to the contract beside it, and says to regenerate. Validating the
pairing on package *name*, which is what it used to do, accepts one version's
plan sitting next to another version's contract and then resolves the second by
answering questions asked about the first. A plan with no `contract` field at
all is refused.

Two things legitimately move the contract's bytes away from the hash its plan
carries, and both are this command's own doing: a reviewer's hand edit, which is
what `resolved-by-edit` exists for, and the promotion at the end. So the review
state records **which plan this review answers** — `state.plan`, the plan's own
`contract` hash — on every write, and never rewrites it. The plan binds when its
hash matches the contract on disk, or when `state.plan` matches it. That is not
a second chance for a foreign plan: the only way `state.plan` acquires a value
is a write that already passed the binding, whose base case is the pristine
match. It is also not approval — the per-resolution hashes still make every
decision recorded before an edit **stale**, so promotion refuses until each is
re-made against the edited bytes. `state.contract`, which tracks the bytes on
disk and moves with them, is deliberately a different field: binding on it broke
as soon as the edit and the re-resolution happened in two invocations, which is
how a person does it.

One case has nothing to bind it and is refused: a contract hand-edited before
any decision was recorded, since there is no review state yet and the edit is
indistinguishable from a swapped plan. Record a decision first, or regenerate.

An edit that writes a *new* positive claim — turning an unknown `callbacks`
sentinel into a callback row, say — additionally raises a generated export
summary question the plan never asked, and promotion refuses on those grounds
until the contract is regenerated. An edit that answers its own item without
changing what the export certifies, such as filling in a callback's `owner`,
promotes normally.

The plan's `generation` block records, per emitted entrypoint, the exact modules
the summaries were derived from — contract-directory-relative paths, `..`
spellings included, with `sha256:` hashes of the exact bytes — plus the generator
identity that produced it and, for a legacy root, the manifest field and artifact
it resolved from. It answers the question the contract's single `artifacts` pair
cannot — *which bytes was this reviewed against* — and nothing loads it as
evidence.

**That record is an attestation, not a reconstruction.** The module list is the
analyzing program's own: the checker asks the compiler for it (`modules`, via
`--emit-module-inventory`) on the same run that emits the contract, and the
generator scopes it to the package being described. It therefore names every
file the analysis opened under the package, declaration files included — a
`.d.ts` the analysis bound an import to determines the summaries exactly as much
as a runtime module does, and leaving it out would be the record lying by
omission.

**What "scoped to the package" means, and what each exclusion costs**
(`packageScope` in `packages/cli/scripts/generate-package-contract.mjs`):

- A file is this package's own when *either* spelling puts it inside the package
  root — the path the analyzing program answered with, or its realpath. Both are
  accepted for the same reason the checker's own inventory filter accepts both:
  TypeScript takes a realpath only where resolution walked a symlink under
  `node_modules`, so a directory symlink *inside* a package (`src -> ../shared`)
  is held under the spelled path while its realpath leaves the root. The record
  names the canonical spelling where it has one, so one file is one module on a
  case-insensitive filesystem too, and the verdict does not depend on which
  machine generated it.
- **A dependency's bytes are excluded**, whether the install hoisted them or
  nested them under this package. They are not this package's bytes, no republish
  of it changes them, and hashing them would bind the record to the install layout
  and to a dependency's version — so two generations over byte-identical package
  bytes would refuse to transfer a review. What the analysis read from a
  dependency is described by *that* package's own contract and closure record.
  The residue — a dependency with no contract of its own — is a named
  approximation in `docs/precision-backlog.md`, not a claim this record makes.
- The compiler's bundled library declarations are excluded: they are not files on
  disk that any record could hash.
- **Anything else the analysis read is noted, not dropped.** A record that
  excludes bytes the summaries were derived from says so.

The generator's own syntax walk
(`packages/cli/scripts/runtime-module-closure.mjs`) survives, and its job is now
narrower and named: it **seeds** the analyzed program's `files` list. It cannot be
dropped, because seeding only the entrypoint makes a published ESM barrel's `.js`
specifiers resolve to the adjacent `.d.ts` files, so the analysis would read
declarations where it now reads runtime bytes. The attestation is both the record
and the **verifier of that seed**.

So the walk still fails closed on every static specifier form it recognizes — a
relative or `#` specifier that names no runtime module inside the package, a
conditional `imports` branch this generation cannot choose between, a
non-literal dynamic `import()`, a module whose bytes are unreadable — and each
problem is then reconciled against the attestation, never quoted blind:

- **The compiler resolved it.** The analysis read a module the walk did not
  seed, so the note is **kept and restated** with the attested path, resolution
  kind, and extension — strictly more than the walk could say.
- **The compiler resolved nothing, and no runtime can either.** The analysis read
  no file for the specifier *and* no existing runtime module inside the package
  answers it, so nothing loads anything here and the note is **dropped**. This is
  the asset-import class (`./styles.css`, `./style.css`), a relative specifier
  naming a file that does not exist, and a specifier escaping the package root —
  whose boundary the dependency contract owns, exactly as a bare specifier's
  does.
- **The compiler resolved nothing, and a runtime still can.** An unselected
  conditional `imports` branch whose targets are real modules on disk: `bundler`
  resolution selects neither, so the record is complete, while Node loads the
  `node` branch and a bundler loads the `browser` one. The record's completeness
  is not in question and the runtime's boundedness is, so this rides
  `runtimeNotes` with the reachable branches named. The distinction is a fact
  about files on disk, never a judgement about a file suffix.
- **A non-literal dynamic `import()`** makes the same claim, on the same field:
  the record names every byte the analysis read, and no module graph can
  enumerate what the runtime resolves.
- **A module the program opened that the walk never seeded**, a module the walk
  seeded that the program never opened, or a module the program opened that the
  record's scope excludes, is its own **note**, in all three directions. This is
  the residue that had no observer before: a walk can disagree with the compiler
  in ways neither side reported, because the process that resolved the modules was
  the other one.

A `notes` entry blocks a review transfer and refuses promotion, exactly as
before. A second field, `runtimeNotes`, carries the claim attestation makes
separable: the record *is* complete for what the analysis read, and something
outside every module graph may still load a module it never read. That refuses
promotion too — it is the other half of RFC 0002 §2 condition 4, raised under its
own `attested-closure-note` blocker kind so the two are countable apart — but it
does **not** block a transfer, because two generations with byte-identical
attested records do describe the same bytes. Both kinds appear on the `contract
artifact binding` checklist section, because either way a human has to look.

**The fail-closed tier below all of that is defence, not a tier users see.** If
the analyzing program's file list were absent, or its module graph reported
itself incomplete, the record would be the generator's own walk, labelled
unattested, blocking every transfer and every promotion; falling back to the
weaker source silently is not a possible outcome. Against the pinned producer
neither shape can occur: a run that cannot write an inventory exits non-zero and
aborts the generation before any contract exists, and the producer builds its
import request out of the program's own inventory answer, so the request is always
a subset of the holdings and `complete` is always true. The code and its tests
pin the contract a future producer must be met with. No generated contract in
this repository has ever carried the sentence.

**One-time re-review after the upgrade.** The record changed shape when
attestation landed: it names the files the program opened rather than the files
the walk found, which adds every declaration file the analysis read. No review
recorded against a pre-attestation record therefore transfers onto a regenerated
plan — `contract review --transfer-from` reports `its runtime module closure
changed` and carries nothing. That is correct rather than unfortunate: the older
record did not name bytes the summaries demonstrably depend on. Regenerate and
re-review once; there is no compatibility path, and a shim that accepted the old
record would be accepting a review of a file set nobody enumerated.

### Probing a generated contract

`contract probe` executes a generated contract's drivable claims against the
package the project actually installed:

```sh
solid-checker contract probe .solid-checker/contracts/reactive-package/solid-reactivity.json
solid-checker contract probe .solid-checker/contracts/reactive-package/solid-reactivity.json --write
```

It sits between generation and both promotion paths:

```
contract generate  ->  contract probe --write  ->  contract verify   (mechanical)
                                              \->  contract review   (human)
```

**It runs the package's code, and its dependencies', in a child process.** That
is why it is a command and not a flag on `contract generate`, whose stated
design property is that package code is never imported or executed: folding
probing into generation would silently convert a static analysis command into
one that runs arbitrary dependency code, including import-time side effects.
Run it where you would run that package's own test suite — a sandbox or
container, no ambient credentials, no network egress. The command isolates what
it portably can: one child process per condition mode, a per-mode timeout
(`--timeout`, default 60s), a scratch working directory the child runs in, and a
staging directory under the project's `node_modules` that is removed afterwards.
None of that is a sandbox, and the command does not pretend to be one.

**The client modes import against a faked browser, and the report says so.**
A module that dereferences `window` while it is being *evaluated* throws
`ReferenceError` in a bare Node process, the worker stops, and every claim of
that entrypoint is undriven — so nothing at all is observed about the package,
including the `kind` claim verification cannot convert. The worker therefore
defines a minimal inert browser surface before it imports anything, in the
`client`, `development` and `production` sessions.

The premise, stated plainly because it is a real weakening: **a claim observed
under the shim is not a claim observed in a browser.** The fake `document`
renders nothing, the fake `matchMedia` never matches, the fake `navigator`
reports `solid-checker-contract-probe` rather than impersonating one. A package
that branches on any of that was observed on the branch the fake sent it down,
and where that distinction could matter the honest state is what the record
says — not what the number implies. So the shim is recorded rather than
assumed harmless: `<contract>.probe.json` gains an `environment` block naming,
per mode, the globals that process invented (`shimmed`) and the ones Node
already provided and it therefore left alone (`present`), and
`<contract>.verify.json` carries the same block forward under `probeReport`.
A promotion built on faked-DOM observations is legible as one.

Four rules keep it bounded.

- **Mode-scoped.** Only modes whose conditions include `browser` are shimmed.
  A `server` import that throws on `window` under `--conditions node` is a
  *truthful* observation of that entrypoint in that mode; faking a DOM there
  would manufacture a pass the package never earns. `server` sessions shim
  nothing, always.
- **Never at generation.** `contract generate` imports nothing at all, so no
  shim exists on the static path. This lives in the probe worker only.
- **Inert-observable.** Every faked value carries a non-enumerable
  `__solidCheckerProbeShim` accessor and the process carries
  `globalThis.__solidCheckerProbeEnvironment`, so a probe body — or a future
  classification that needs to know the DOM was fake — can ask. Both are
  non-enumerable, so a package's own feature detection sees exactly what it
  would see in a browser and nothing extra.
- **Empirical, not speculative.** The list is
  `window document navigator self location screen history localStorage
  sessionStorage matchMedia requestAnimationFrame cancelAnimationFrame
  getComputedStyle MutationObserver ResizeObserver IntersectionObserver`,
  derived from what the corpus's failing packages actually reach for rather
  than from what a browser happens to have. The same rule governs each fake
  object's members: `document.readyState`, `navigator.userAgent`,
  `node.ownerDocument`, `document.defaultView`, `style.getPropertyPriority`
  and the rest are there because packages in the corpus reached them, plus
  each one's immediate structural neighbour — a node carrying `firstChild` and
  no `childNodes` is a node that throws one line later.
  `--no-environment-shim` reproduces the bare-Node environment, which is what
  makes the shim's effect on a measurement separable from the engine's.

  The back-references matter more than they look. A package that reaches
  `node.ownerDocument.addEventListener` from a *deferred* callback throws
  inside a timer, and an uncaught exception in a timer kills the whole worker
  process rather than one probe — taking every remaining claim of that mode,
  `kind` observations included, with it. So `node.ownerDocument === document`
  and `document.defaultView === window` hold in the fake as they do in a real
  DOM.

  A mutator that quietly drops what it was given is the sharpest way this rule
  gets broken, and it did: `history.pushState`/`replaceState` were no-ops, so
  `history.state` stayed `null` forever. `@solidjs/router`'s `saveCurrentDepth`
  calls `replaceState({ ..., _depth }, "")` and reads `history.state._depth` on
  the very next line, unconditionally, at import time in every
  browser-conditioned mode — so the old no-op manufactured a crash
  (`Cannot read properties of null`) no browser would ever produce. (The
  checked-in ecosystem report still records that `_depth` crash; it predates
  this shim change.) Both mutators now really set `history.state` — cloned
  the way the spec's shared push/replace steps require, so a package can
  never observe aliasing or throw-free acceptance of an uncloneable state —
  and `length` follows the same spec where implemented: it starts at 1 (it
  was 0, a value no browser reports), `pushState` adds an entry,
  `replaceState` does not. `go`/`back`/`forward` stay inert, so after such a
  traversal `length` can exceed what a settled browser would report — a
  documented approximation, not a claim. `document.head.append`/`prepend`
  were simply missing — real, variadic `Element` methods a `document.head`
  never had in this shim. `@solidjs/start-devtools`'s development build,
  which mounts its own style tag that way at import time, was the case that
  surfaced the gap in this session's probe runs (the checked-in report
  records only that its entrypoint import threw).

An import that still throws with the shim in place is unchanged: undriven,
`import-failed`, with the throw as its reason. And the shim buys nothing at all
for a `typeof window === "undefined"` guard — `typeof` on an undeclared
identifier never threw. For those modules the shim *redirects* rather than
rescues: a package that took its server path in every earlier run now takes its
browser path. That is the sharpest reason the shimmed list is data.

**A worker stops at its first throw and the mode is restarted for what is
left.** That is a correctness requirement, not a performance one: Solid 2.0's
development build halts the reactive system permanently on an uncaught error —
*"No further updates will be processed"* — so every probe after a throw in the
same process observes a runtime where nothing ever re-runs, and a genuinely
tracked callback reads as inline. Restarting is the only way to un-halt it; each
restart answers at least the probe that stopped the previous one, so the loop is
bounded. A whole-process failure — a crash, a timeout, unreadable output — names
no particular probe, so the rest of that mode is recorded undriven rather than
retried.

**An asynchronous throw stops the process, not the mode.** Package code the
probe set running — a deferred callback, a promise left rejected — throws
outside every `try` the worker has. The process used to die with status 1 and
an empty stdout, so the parent had *no* results for that mode: every probe
already answered was discarded, and because a whole-process failure names no
probe to retry past, the mode ended there. The worker now answers with what it
observed, `completed: false`, and the abort reason, so the parent restarts for
the remainder exactly as it does after a synchronous throw. The reason is
reported and never attributed to a claim — nothing says which probe scheduled
the work that threw. Two corpus rows lost a verification to the old behavior.

Probing changes **no** evidence kind. A probed contract is still `inferred` and
still certifies nothing. What the probed rows it writes are *for* is
[`contract verify`](#machine-verified-contracts), the mechanical promotion that
certifies exactly them and converts everything else away.

**What is drivable, and what is not.** A contract records a callback's exact
parameter index and nothing about the other parameters, so the driver
synthesizes the rest from the contract's own structured vocabulary and never
from a type: the probed slot gets the probe callback, a slot another
`callbacks[]` row names gets a no-op function, a slot a `parameter-member`
reactive read names gets an empty object, and every other slot gets `undefined`.
There is no ladder of retries — trying `{}`, then `[]`, then `0` until something
completes would make drivability depend on which shape happened to survive.
Drivability is therefore **empirical**: the call is attempted, and a claim whose
call could not be constructed or did not reach the callback is recorded
`undriven` with the exact reason. Undriven is never a failure and never
evidence; it is the measurement of how far the boundary reaches.

Claims with no probe form at all are recorded undriven with a standing reason:
callback `owner` rows (no observation distinguishes inherited from created
ownership — permanently out of reach), callback `arguments` descriptors, reactive
reads, owner requirements, `asyncBehavior` (which has no evidence slot in the
schema, so a driven observation could not be recorded), nested return leaves,
and `returns` of kind `store-path`, `argument`, or `callback-result`.

**What a probe observes.** `inline`, `deferred` and `tracked` classify
attribution, so the export call sits inside a memo, the probe callback reads a
signal, and the signal is then written: a call site that re-ran owns the reads
(`inline`), a callback that re-ran alone holds its own subscription (`tracked`),
a callback that ran synchronously and neither re-ran is `inline` with the
listener cleared, and one that ran only after the call returned is `deferred`.

**Three things have to be ruled out before those readings mean anything.**

*A re-run has to have been caused by the write.* Between the baseline and the
write the body settles once more with **nothing written** — the control
interval — and reports the count after it. A callback that ran again there ran
again without a write, and two ordinary shapes do: `@corvu/utils`' `afterPaint`
is a double `requestAnimationFrame`, which the worker shims to nested timers, so
its *first* run lands after the baseline was taken;
`@solid-primitives/timer`'s `createTimeoutLoop` reschedules itself and runs again
across every interval whatever is written. Both used to read `tracked` — a
callback holding no subscription at all, reported as a package defect against
the `deferred` claim that was right. With the control interval, a callback that
ran again there and again after the write is unattributable (`undriven`: nothing
separates what the write did from what was going to happen anyway), and one that
ran again there but *not* after the write proves the write caused nothing —
`deferred`, confirmed.

The reason recorded with that withdrawal says what was counted, not why it
happened. `raf(() => raf(() => createEffect(cb)))` — start tracking after paint,
an ordinary idiom — is a genuinely `tracked` callback and produces the same three
counts as `createTimeoutLoop`, so the withdrawal is forced but *"it schedules
itself"* would be a claim about the package that the counters cannot support.
Undriven reasons are published per claim in `<contract>.probe.json` and
aggregated across the corpus, so a reason is held to the same standard as a
verdict.

*A first run is not a re-run.* The control interval narrows that window; it does
not close it. A callback that had not run by the time of the write — not during
the call, and not across the control interval — held no subscription to the
probe's signal, because the only read of that signal is in the callback body. Its
run in the write interval is therefore a first run, and the write cannot have
caused it. A package that defers by roughly three macrotask hops
(`setTimeout(cb, 3)`, a triple `requestAnimationFrame`, a promise chain into a
timer) lands exactly there, and used to be reported as defective against the
`deferred` claim it honours — on some runs and not others, since which interval a
first run lands in is a property of the machine's load rather than of the
package.

It is not `deferred` either, and the asymmetry is the point. The `deferred`
reading of `rb 0, rc 1, ra 1` is earned: the callback ran, and so read the
signal, *before* the write, and the write then did not re-run it, which is an
observed absence of a subscription. No such test exists for a run that happens
only after the write, and a callback whose subscription is established late —
`raf(() => raf(() => raf(() => createEffect(cb))))` — runs exactly once, in the
write interval, having never run before it, and is genuinely `tracked`. Measured
on both audited releases, the two shapes produce identical counters, so the
observation names no mode at all.

*A call-site re-run is not proof of `inline`.* It is implied by `inline`, and
the converse was assumed. The site also re-runs when the export reads its own
tracked derivation of the callback **during the call**, which subscribes the
caller transitively: `mergeProps({...defaults}, props)` followed by a read of a
defaulted member is exactly this shape, and so is an export that invokes the
parameter once directly and once inside an effect. What separates those from a
genuine `inline` is that the callback then ran *more often than the site
re-invoked the export*, so a subscription the call site does not own re-ran it;
which reads belonged to the site is not something these counters can settle, and
the observation is `undriven`. The residual conservatism is deliberate: an
export that invokes its callback twice per call is `inline` and lands undriven
here, because those counters are also what a transitively subscribed site
produces, and failing closed on the ambiguity is the safe direction.

Where the contract states `returns.kind: accessor`, the returned accessor is
read to force a lazily computed export — inside a fresh memo created under
`untrack`, so the reads get a computation to land on without the call site
borrowing a subscription it did not earn. `returns=accessor` itself is confirmed
by planting the signal read in a callback the contract states and observing the
returned value re-read after the write; `typeof value === "function"` is a
sighting, not an observation of reactivity, and does not confirm the claim.

Forcing a callback that way costs one piece of certainty, and the driver gives
it up rather than spend it: when a callback ran only because the driver read the
returned accessor, an execution mode that disagrees with the claim is recorded
**undriven**, not failed. Which computation owns those reads is then partly a
property of the read scope the driver created — Solid 1.x's `createSelector`
comparator runs synchronously inside the selector call while its reads register
on the selector's own computation — and asserting a package defect on the
strength of the driver's own scaffolding is the wrong-is-dangerous direction. A
mismatch on a callback the export invoked without that help is a failure.

An entrypoint that *is* a reactive runtime drives its own probes. Solid 1.x
resolves `.` to `dist/dev.js` in development while `./jsx-runtime` stays on
`dist/solid.js`, so a signal made by one and a memo created by the other belong
to different schedulers and nothing tracks anything; the generic form of the
rule the bundled 1.x worker records by hand is that a namespace exporting
`createSignal`, `createMemo`, `createRoot` and `untrack` is used as its own
runtime. For an ordinary package the project's `solid-js` drives, which is the
same instance the package itself resolved.

**A runtime that re-runs nothing observes no execution mode at all.** Every
callback observation above is a *differential* measurement — write, settle, see
what ran again — and that presupposes a runtime in which something can run
again. Both audited releases resolve `node` to a server build where nothing can:
1.9.14's `dist/server.js` returns `[() => value, setter]` from `createSignal`,
computes a memo once, and has an empty `createEffect`; 2.0.0-rc.1's makes
`flush()` a no-op. The worker builds its scaffolding out of that same runtime, so
in such a session the scaffolding is inert: `tracked` is not merely unobserved
but **unobservable**, and `inline` and `deferred` are indistinguishable from it
and from each other.

So the runtime is asked rather than assumed. Before it drives a probe the worker
runs a **capability self-check** with the same runtime the probe will use —
create a memo over a signal, write, settle, see whether the memo ran again — and
stamps every driven observation with the answer. The driver records **every**
`callbacks[n]` observation of a runtime that re-runs nothing as `undriven`, and
suppresses discovery findings from it, because the execution mode in such a
finding's claim string would be whatever the inert scaffolding defaulted to.
`kind` claims are unaffected: they read `typeof`. `returns` claims keep their own
verdicts, because passing one already requires an observed re-read that an inert
runtime cannot produce, and because *"the call returned an object"* stays a real
observation.

Four properties of that design are load-bearing.

- **It withdraws passes, not only failures.** In an inert runtime an `inline` or
  `deferred` claim *matches* — the counters can produce nothing else — and
  recording that as a pass turns into `probed` row evidence and then into a
  verified contract. Withdrawing the unearned passes is the half that makes this
  a correctness fix rather than a way to raise a number.
- **It is name-free.** Nothing tests for `server`, `node`, a version or an
  artifact path. The property that matters is whether the artifact a mode
  resolved is reactive, and that does not follow from the mode's name.
- **It is per runtime, not per session**, because one session holds more than
  one. Probing `solid-js@1.9.14` under `--conditions node`, `.` resolves to the
  non-reactive `dist/server.js` while `./jsx-dev-runtime` resolves — through a
  single unconditional target in the manifest — to the fully reactive
  `dist/solid.js`, which drives its own probes. A per-session answer taken from
  the project runtime would discard the jsx-dev-runtime observations that are
  genuinely attributable; taken from the self-driving namespace it would certify
  the server build's inert ones.
- **It is recorded, not just applied.** Attribution comes from the per
  observation stamp for the reason above, and the answer the worker measured for
  the runtime that drove that mode's ordinary packages is written down per mode
  as `sessions.byMode.<mode>.runtime` in `<contract>.probe.json`, from where the
  verify sidecar carries it forward. Without it a reader of a report sees a batch
  of `undriven` rows with a per-claim reason and has to reconstruct the mode-level
  fact; with it the report says which modes were measured inert. `null` means no
  process of that mode got far enough to measure a runtime, which is not the same
  fact as *"measured, and nothing re-ran"*.

Every driven claim runs in every applicable condition mode, with an initial and
a subsequent call. The applicable modes come from the entrypoint's recorded
conditions and each variant's conditions; `--modes` narrows them further. There
is no per-package `probeModes` equivalent. Until the self-check existed, a claim
a package states only for some environments had to be narrowed by hand with
`--modes`, and probing Solid 1.x's `node` build against a client-semantics
contract reported the divergence as a *failure* — which is why 82 of the corpus's
218 failing claims were `server`-mode-only, and why the same modes were handing
out unearned `inline` and `deferred` passes. A mode whose runtime cannot re-run
contributes nothing in either direction for **callback execution claims**: it
cannot provide callback attribution evidence or a callback-mode contradiction.
`kind` claims still read `typeof`, and `returns` claims still keep their direct
return-shape verdicts; those observations do not require a reactive re-run.

Those two families keep their verdicts in **both** directions, which is the part
worth stating exactly rather than rounding to "an inert mode contributes
nothing": such a mode can still produce a **failure**. A `kind` mismatch is one,
and so is a `returns=accessor` claim whose call returned a non-function — a value
no amount of re-running would turn into an accessor. Both are sound from an inert
runtime because both are `typeof` observations rather than differential ones, and
a mode that resolves an artifact of a different shape than the one the generator
analysed is a real environment mismatch rather than an artefact of the probe.
Whether that is reachable at all was probed and **not** demonstrated: on both
audited releases no export differs in `typeof` between the client and the server
artifact — the differences are presence, which becomes `export-missing` and so
undriven — and every `isServer` early return in the sampled corpus packages
returns a function. So the claim is that `--modes` is unnecessary for correctness
in practice, not that an inert mode is inert for every claim family.

Thus `--modes` is a way to bound callback-probe time rather than a correctness
requirement for those claims. A claim that passes in its remaining modes still
passes: undriven callback modes are ignored when a claim settles.

**Probes confirm; they never write behavior.** `--write` records passing modes
as `probed` row evidence on claims that already exist, exactly as the bundled
suite's `--write` does, and an existing `reviewed` or `inherited-from` marker is
never overwritten. A behavior observed that the contract does **not** state is
an `INCOMPLETENESS` finding — the one automated check anywhere here that can
contradict a negative claim — and it fails the run rather than becoming a row.
The driver plants a callback in the first two parameters an export's summary
leaves unclaimed to elicit exactly that, for `value` summaries as well as
function ones — a `value` summary is the maximal negative claim, and exempting
it from its own falsifier was a claim checking itself. A failed probe or an
incompleteness finding leaves the contract untouched and exits non-zero.

**A write supersedes.** A `probed` marker is durable, so the rule above is only
half of it: what happens to a marker the *current* run no longer earns. When
this run drove the same claim and it did not pass — the package changed, the
import now throws, `--modes` narrowed the run — the row's marker is refreshed
with what this run observed, or removed when this run observed nothing. Each
removal prints one `superseded` line and lands in the report under
`superseded`, with the claim and the marker it replaced. A claim this run did
not attempt keeps what it had; [`contract
verify`](#machine-verified-contracts) separately refuses to certify a marker
its own report does not witness, so a stale observation is caught on both
sides. `reviewed` and `inherited-from` markers are still never touched: they
are not this command's to move.

**`--no-discovery` is for investigation only.** It skips the negative-claim
falsifier, the probe report records `discovery: {"enabled": false}`, and
`contract verify` refuses such a report outright. It bounds probe time while
you are looking at a package; it cannot produce a report anything certifies
from.

**Call counts are measured.** `evidence.calls` is the number of times the probe
actually invoked the export, counted by the worker. It used to be a per-probe-
type constant, so a `deferred` claim — whose whole shape is that the call site
does *not* re-run — recorded `calls: 2` for one invocation.

`--write` moves the contract's bytes, and a review plan is bound to the exact
bytes it was written beside — so probing belongs *before* any review decision.
When the plan beside the contract is still the one generation wrote and the
review state has answered nothing, the write re-binds that plan to the new bytes
after checking that the probed evidence raised no review question the plan does
not list. Once a review has recorded a decision or a promotion, `--write`
refuses outright: those decisions are bound to the bytes on disk, and moving
them underneath a recorded answer is the silent re-blessing the binding exists
to prevent. Regenerate and probe the fresh document instead.

**The report.** `<contract>.probe.json` records, per `(entrypoint, export,
claim)`, the claim family, the modes attempted, the modes passed, the measured
call counts, the synthesized argument vocabulary, and every undriven claim's
reason — plus the discovery state, the markers a write superseded, and the
identities the result is a function of: the installed version and npm
integrity, the generator identity the review plan carries, the probe driver
identity, and the resolved dialect and Solid release. It also records the
`environment` block described above and a `sessions` block — how many worker
processes each mode cost and how many of those were restarts after a throw.
Restarts are not failures, but a mode that needed hundreds of them is the shape
behind a slow or timed-out probe, and until this the count was visible only as
an unexplained duration. Nothing loads it and
nothing certifies from it. It is the audit trail for what the machine believed
and could not confirm, and it is the measurement of how much of a real
package's contract a machine can reach.

The `family` label is the one RFC 0002's taxonomy assigns, and it now agrees
with what `contract verify` does with the row. `reactiveReads[]` and
`ownerRequirements[]` are family **(A)** — proven from compiler facts, kept at
verification, and undrivable only because no probe claim string names them.
They used to be reported as **(C)**, the family whose definition is "converted
to the unknown sentinel before promotion", while verification kept them; the
report and the code told different stories about the same row. Callback
`owner` rows, callback argument descriptors, nested return leaves, store-path,
`argument`, and `callback-result` returns, and `asyncBehavior` are family (C)
and do convert.

**`returns=accessor` needs caching, not just reactivity.** The observation
plants its signal read inside the callback the contract states, so a plain
forwarding closure — `(cb) => () => cb()` — re-reads that signal on every read
of the returned value and looks reactive by transitivity. The distinguisher is
caching: the body reads the returned value twice inside one evaluation of a
single tracked scope and counts how many times the planted callback ran. A memo
accessor recomputes at most once; a forwarding closure runs the callback once
per read, and the claim is `undriven`. An *uncached* derived accessor — 1.x
`mapArray`'s plain tracked function is the real example — lands undriven too.
That is the safe direction: the claim stays unproven and its domain converts,
rather than being certified by a property a forwarding closure also has.

The probe runtime follows the installed release, the same way the checker's own
dialect detection does: the nearest `node_modules/solid-js` above the package
under probe. 2.0 settles with `flush()` and detaches a probe's write from the
test root; 1.x settles by yielding to a macrotask. Unlike the analyzer, probing
has no safe default — settling a 1.x runtime with 2.0 semantics observes the
wrong thing — so a missing or unclassifiable `solid-js` is a refusal, never a
fallback.

### Machine-verified contracts

`contract verify` promotes a probed contract to `evidence.kind: "verified"` with
no human decision anywhere in the path:

```sh
solid-checker contract generate --package-root node_modules/reactive-package \
  --output .solid-checker/contracts/reactive-package/solid-reactivity.json
solid-checker contract probe  .solid-checker/contracts/reactive-package/solid-reactivity.json --write
solid-checker contract verify .solid-checker/contracts/reactive-package/solid-reactivity.json
```

`verified`'s reserved meaning here has always been *"mechanical
artifact/surface/behavior checks passed"*, and until this command nothing in the
repository wrote it. The design is
[RFC 0002](rfcs/0002-machine-verified-contracts.md); read §2 and §3 before
changing any rule below.

**Why its own verb.** `contract review` records a *human review*, one decision
per plan item, and its `generated-summary` item exists so that no row reaches
`reviewed` evidence without a human decision naming its export. Mechanical
promotion does not weaken that invariant — it declines to enter that tier. So
`contract review --promote verified` still refuses, and names this command
instead of only saying no.

**What it certifies, and what it destroys.** The rule is one sentence:

> A machine may certify exactly what it proved or observed. Every other positive
> claim it holds must become the unknown sentinel before promotion. Never a
> guess, never a downgrade that hides.

*Proved* means the generator derived it from exact compiler facts and already
fails closed on it: the negatives-by-omission, `reactiveReads`,
`ownerRequirements`, and the `variants` structure. Those pass through untouched,
because the generator emits `{"status": "unknown"}` where those facts are *not*
exact — so an emitted row is the proven case by construction. *Observed* means a
`probed` row marker that `contract probe --write` put there, covering **every
mode the claim is stated for**: an entrypoint with no `conditions` states its
claims for all four modes, so a row probed only under `browser` does not cover
it. Narrowing the stated modes instead would claim semantics for an environment
nobody observed.

**Observed by *this* run.** A `probed` marker is a durable artifact of some
earlier run, and it used to be self-certifying: the document said "observed"
and nothing asked which run observed it. So a healthy probe, a later probe that
observed nothing at all, and a verify certified every marker the first run had
written. Every `probed` marker in the document must now be *witnessed* by the
consumed report — a passing claim of the same identity, covering at least the
marker's modes. An unwitnessed marker converts its domain exactly like an
unprobed row, and is listed in the sidecar under `staleProbedMarkers`.

Conversion rather than a blocker is the deliberate choice here. From this run's
point of view an unwitnessed marker and an absent one are the same epistemic
state — nothing in the consumed report observed the claim — and the sentinel is
the spelling this design has for that. Blocking would also make an honest
narrow run unrecoverable: a `--modes client` report could never verify
anything, rather than verifying less, and "verify less" is the direction the
whole rule takes.

Everything else converts, per **domain**:

| Claim | Verified outcome |
| --- | --- |
| an omitted effect field (the negative claim) | kept — the generator proved it, and no probe can ever verify a negative |
| `reactiveReads[]`, `ownerRequirements[]`, `variants[]` | kept |
| `kind: function` / `value` | kept **only when probed in every stated mode**; otherwise its entrypoint is refused and omitted (see below) |
| `callbacks[].execution` | kept when probed and witnessed in every stated mode |
| `returns` (top-level `accessor`) | kept when probed and witnessed in every stated mode |
| `callbacks[].owner` | converts the whole `callbacks` domain — permanently out of a machine's reach |
| `callbacks[].arguments[]` descriptors | converts the whole `callbacks` domain |
| `returns` `store-path`, `argument`, `callback-result`, or nested `elements`/`properties` leaves | converts `returns` |
| `asyncBehavior` | converts, always — no probe claim string, and no evidence slot in schema v1 to record one in |
| any row carrying `inherited-from` evidence | converts its domain, at the top level **and inside every variant** |

**`kind` blocks, because it has no sentinel.** Schema v1 requires `kind` on
every export summary and its two values are the whole vocabulary, so "not
proven" is unsayable there: no weaker document exists to promote. That left it
exempt from the conversion rule and therefore from every check, and the
protection the code claimed — "a runtime kind that disagrees is a failed probe"
— is vacuous when the probe observed *nothing*. A contract none of whose claims
were driven verified anyway. A `kind` claim not probed-passed in every mode its
export is stated for is therefore not certifiable. The consequence is
deliberate: **a package this checker cannot import cannot be machine-verified at
all.** It can still be reviewed, and a human's reading of an unimportable
package is exactly what the reviewed tier is for.

**The unit of that refusal is the entrypoint.** An entrypoint whose `kind`
claims this run did not observe is refused and omitted from the promoted
document — exactly what [`contract generate`](#refused-entrypoints-versus-failed-generation)
does with an entrypoint it cannot certify — and the other entrypoints are still
promoted, so one unimportable subpath no longer costs a package its other
twenty. A refused entrypoint is absent from the contract, so a consumer
importing it gets an explicit uncertifiable result rather than a claim nothing
observed.

Three artifacts say so, and between them is what keeps "it can still be
reviewed" true of a *partial* promotion:

- `<contract>.verify.json` names each refusal under `refusedEntrypoints` with
  the exports whose `kind` was unobserved, and counts them in
  `summary.refusedEntrypoints`, whose sibling `summary.exports` counts the
  promoted document — so the two figures together say the document is smaller
  than the draft it came from.
- **the rewritten review plan carries a `refused-entrypoint` item** for each
  one, naming the exports the promotion dropped. The plan is re-derived from the
  promoted bytes, so without that item the subpath would leave the document
  *and* the plan — and `contract review` never reads the verify sidecar. It is
  the same item kind, in the same section, that generation raises for an
  entrypoint it refused, for the same reason: a partial contract must never be
  silent about what it omits.
- stdout prints one line each.

**A document where no entrypoint would certify anything is still refused
whole.** That is the actual test rather than "every entrypoint was refused": an
entrypoint with an empty export map certifies nothing either, and raises no
refusal to be counted. The blocker leads with the same phrase the per-entrypoint
lines carry and enumerates the refusals after it, and each refused entrypoint
still gets a line of its own naming its unobserved exports and modes — a refusal
sidecar's `blockers.raised` is the only durable record of a refusal, and one
line naming five of ninety-one is not one. See
[RFC 0002 amendment A9](rfcs/0002-a9-kind-has-no-unknown-form.md) for the
measurement behind the rule and the three relaxations it rejects.

**An exported class is `kind: "function"`.** Constructability is not
callability: Type Facts derives `Callability` from `GetSignaturesOfType(…,
SignatureKindCall)`, a class type carries construct signatures and no call
signature, and so every exported class answers `nonCallable` there. At runtime
`typeof C === "function"`, which is exactly what `kind` describes and exactly
what the kind probe measures. `Constructability` — the same walk asked of
`SignatureKindConstruct`, demanded at the same span — is the fact that answers
it, and it is transparent through an alias, an import and a re-export because
the type is, so `export { Watcher }` and `const Alias = SomeClass` need no
symbol hops of their own. `@tanstack/solid-db@0.2.37` alone failed 102
`kind=value` claims, every one of them an error class.

Its `callbacks` domain then fails closed. The generator summarizes function
declarations, not construct signatures, so nothing carries what a constructor —
the class's own or the one it inherits through `extends` — does with the
arguments a caller passes; a consumer reads `new Store(onChange)` through the
same contract path as `store(onChange)`, so the omitted (negative) claim would
certify inertness the class can contradict. The sentinel stays
demand-sensitive: constructing with no callable argument is still clean.
`fixtures/package-contracts/exported-class` pins the three resolution shapes
and the `kind: "value"` negative.

**A class expression counts, because that is what ships.** Rolldown, esbuild
and tsdown all lower `export class C {}` to a `var` bound to an *anonymous*
class expression, and re-export it by specifier. In that artifact no
class-name span covers the exported binding and no alias hop reaches a class
declaration, so reading only declaration kinds and class-name spans left every
bundled class `kind: "value"` — 45 of the 53 failing `kind` claims in the
corpus measurement, across Solid Primitives (`ReactiveMap`, `ReactiveSet`,
`TriggerCache`), `@solidjs/web` (`ResponseEnvelope`), `@kobalte/core`
(`SelectionManager`) and `@tanstack/*` (`AsyncBatcher`, `Debouncer`, `Queuer`,
`Throttler`, the `*DevtoolsCore` family). The declarator's name types as the
constructor, so `Constructability` answers all of them — including the two no
syntactic search could reach: an IIFE-wrapped class whose initializer is a
*call* (`@solidjs/web`'s `ResponseEnvelope`) and a class reached only as a tuple
element type declared in another package (`@tanstack/*`'s `*DevtoolsCore`).
`fixtures/package-contracts/class-expression-kind` pins the entry-file, `.js`
barrel-hop and package-boundary shapes.

**Some spans are not the export's value: a class declaration's.**
`export class C {}` has no export specifier — the exported name *is* the
declaration's name — and the compiler's type there is the class's *instance*
type, which honestly answers `nonCallable` and `nonConstructable` because an
instance is neither. `export default class {}` is the same problem in the
spelling that has no name at all: the export records the `class …` node's own
span, and the facts there describe the instance type too. The producer's ADR
0020 pins that by test and says outright: demand at the export-specifier span,
never at a declaration name. Both shapes are therefore decided by the
declaration they are (`AstFacts::declares_class_at`, which answers for a class's
binding name and for an anonymous class node), which is a span-addressing rule
rather than a class-ness heuristic: `class C {}` binds the constructor by
language definition — named or not — and a bundler that lowers the declaration
away leaves a *declarator* name, which types as the constructor and is decided
by the facts. `fixtures/package-contracts/exported-class` is the regression pin
for all of them: `DirectError` for the named declaration, and the
`./anonymous-default` and `./anonymous-extends` entrypoints for the anonymous
one. Each published `kind: "value"` — a false certified negative for a
constructor — while every other export in that fixture stayed correct.

**A `namespace` member is not an export.** A declaration inside a `namespace`,
`declare module` or `declare global` body binds a member of that namespace
object, not a name in the enclosing module: `export namespace Config { export
const inner = 1 }` publishes `Config` alone, and `import { inner }` does not
resolve. Both surface enumerations — the entrypoint's export names and the
project-wide export map — iterate `AstFacts::module_level_exports` and skip
bindings inside a module block for that reason;
`fixtures/package-contracts/namespace-export-surface` is the pin, including the
merged class+namespace shape, whose class keeps `kind: "function"`.

**When no closed type answers `kind`, the entrypoint is refused.** A bare
`kind: "value"` summary is the *maximal certified negative*: `validate_export`
bars a `value` summary from carrying any claim domain, known or unknown, so it
asserts that the export reads nothing reactive, returns nothing reactive,
invokes no caller-supplied callback and requires no owner. Publishing one
therefore needs a proof that the export is not a function, not merely the
absence of a proof that it is. `Unknown` (an `any`, `unknown`, `never` or
error type — what an untyped dependency leaves behind in a published `.js`
artifact) and `Mixed` (a union holding both a signature-carrying and a
signature-less constituent) are both the absence of that proof on *either*
fact, and treating either as `value` is how `@solid-devtools/locator@0.16.7`
published "invokes no caller-supplied callback" for `addClickInterceptor(fn)`
and `addHighlightingSource(fn)` — the remaining 8 of those 53 rows. Nor does
`Mixed` on both compose into a per-constituent proof: the producer aggregates
the two independently, and `(() => void) | number | (new () => X)` answers
`Mixed` twice while still holding a constituent that is neither. An **absent**
fact refuses on the same reasoning: both are demanded at every export specifier
and every exported declaration name, so absence at one of those spans is the
producer finding no node to classify — missing evidence about the export, not an
answer about its type. Since `kind` has no sentinel, the honest outcome is the
existing refusal path below: the entrypoint is omitted and a consumer of it gets
an explicit uncertifiable result.

**`nonCallable` alone is not the same claim as "not a function", and neither is
`nonConstructable`.** Each says the type lacks one signature kind, and a class
lacks the call one. Only the two closed negatives *together* are a `value`
proof. That is what discharged the destructuring-pattern refusal this rule used
to carry: `const { Inner } = Container` binds a *member* and `const [Core] =
pair` an *element*, so no syntactic search could reason about either, and
`nonCallable` proved nothing about a binding whose class-ness question was never
asked. The type answers the pattern directly — `(class Named {}).name` is a
string and both closed negatives hold, while a static class member and a tuple
element whose type is a class are `Constructable` — so
`fixtures/package-contracts/class-expression-kind`'s `./destructured`
entrypoint now *publishes* two `value` claims where it used to be refused.

Schema 15 closes the signature-less `Function` residue with
`Callability::UntypedCallable`. `Function`, `CallableFunction`,
`NewableFunction`, aliases/interfaces based on `Function`, and intersections
containing it now prove runtime `kind: "function"` without claiming a readable
signature. Broad types such as `object`, `{}`, and `Record<string, unknown>` are
not members of that family: they admit non-function values and correctly keep
the closed negative pair. `fixtures/package-contracts/function-supertype-kind`
pins both sides of that boundary. The producer and consumer migration, plus the
remaining signature and union limits, are recorded in
[docs/precision-backlog.md](precision-backlog.md).

**A raise to `kind: "function"` always leaves `callbacks` unknown.** Every
raise reaches `raised_function_export` with a summary that still said `value`,
which is exactly the state in which no function body was summarized for the
export — a class never has one, and a callable binding that had one would
already carry its analysis's claims and never be raised. Publishing the raise
with every domain absent therefore certified "invokes no caller-supplied
callback, returns nothing reactive, requires no owner" for a body this run
never read. The raise corrects a wrong `kind`; it cannot also be read as
evidence about behavior.

**Two boundaries on the refusal.** A summary **carried from a dependency
contract** keeps its kind, but only on provenance the document cannot fake
(`PackageContract::kind_claims_are_trusted`): either this generation run
produced that contract itself from the dependency's own installed sources under
this same rule — the generator passes those with `--generated-contract`, never
`--contract` — or its `evidence.kind` records that a human or a verifier stood
behind its claims (`reviewed`, `verified`, `trusted`, `attested`). Re-deciding
such a kind here, with the dependency's implementation outside the project and
its specifier consequently `any`, would refuse exactly the entrypoints that
already have the better answer.

A contract with *neither* provenance is a document of unknown origin.
`dependencyContracts()` discovers `node_modules/<dep>/solid-reactivity.json`
by walking upward with no flag from the user, so an `inferred` contract written
by any earlier solid-checker — including one with the `Unknown ⇒ value` defect
this rule closes — arrives indistinguishable from a reviewed one. Its `kind`
goes through this decision like a local claim: corrected where the consumer's
own facts prove it, refused where they do not. Every *other* claim in such a
contract is used exactly as before; only `kind` is gated, because only `kind`
has no unknown sentinel to fall back on.
`fixtures/package-contracts/carried-value-kind` pins all three directions.

And a location carrying **no callability fact at all** keeps today's `value`:
`demand_plan` requests callability exactly where it requests a type
descriptor, so absence there is missing evidence about the span rather than an
answer about the type, and refusing on it would refuse for a demand-coverage
accident. That residue is recorded in
[docs/precision-backlog.md](precision-backlog.md).

**An unmarked type-only re-export is omitted, not refused.** `export type
{ T }` and `export interface T {}` say what they are, and generation drops them
by that marker. `import { Options } from "./types.js"; export { Options }` is
equally legal and says nothing at the export site — and at that span no
producer fact separates it from a value whose type is unresolvable: callability
is `Unknown`, `runtime_identity` is empty, `reference_space` is structurally
`Neither`, and the declaration kind is the catch-all `"declaration"` for both.
Left to the kind decision it refused the whole entrypoint, costing every real
export beside it for a name with no runtime existence to describe. So
generation follows the same relative re-export and import chain it already
walks for dependency summaries, and omits a name whose every export along that
chain is `type_only`. Fail-closed by construction: a chain that leaves the
project (a bare specifier, an unresolvable relative path) or that the walk
cannot see (`interface T {} export { T }`, where no `type_only` specifier
covers the local declaration) proves nothing and stays with the refusal.

**A summary-level marker does not outlive its claims.** `writeProbeEvidence`
computes the export summary's own `probed` marker from the `callbacks[]` rows
and the top-level `returns`. When those claims are converted here — or deleted
by a review that certified them absent — the marker would assert an observation
of claims the document no longer contains, and a row with no evidence of its
own inherits it. It is recomputed in both paths and survives only when the
summary still states at least one probeable claim and every one of them carries
a `probed` marker.

**An inherited summary converts all the way down.** A summary whose evidence is
`inherited-from` is another package's claim, so every domain it carries becomes
the sentinel. The recursion used to drop the inheritance on the way into
`variants`, which let the exact per-environment claims — the ones an
environment-aware consumer actually selects — pass through certified. Its
omitted domains are still certified negatives; that residue is recorded in
[docs/precision-backlog.md](precision-backlog.md).

Conversion is per domain and not per row because the sentinel is a *field
value*: one unconfirmable callback row converts the export's entire `callbacks`
field, taking a probed sibling row with it. That is deliberately lossy, and it
is the price of the sentinel being the only "not proven" spelling schema v1 has.
It is also already how the generator behaves across conditional branches, where
`mergeClaimRows` returns unknown when either side is unknown — and where two
branches that each *prove* a different `returns` or `asyncBehavior` write the
sentinel into the environment-unaware base rather than refusing the entrypoint.
The exact per-branch behavior is still emitted, as `variants` beside that base;
what the base cannot say is that one of them holds everywhere. solid-js 1.9.14's
`Show` is the shape: it returns its `props` argument in the server build and a
memo accessor in the client build.

The `inherited-from` row is the one conversion RFC 0002 does not name. It is
fail-closed by decision: the claim came from another package's contract, whose
*tier* nothing at this point can check, so this machine neither proved nor
observed it. Relaxing it needs consumer-visible tiering (RFC 0002 unresolved
question 6) to exist first.

Finally, every surviving `inferred` row marker is **deleted**, exactly as
`--promote reviewed` does: certification rejects any inferred row inside an
otherwise-certifying document, while a row with no evidence of its own inherits
the document's. Writing `verified` onto each row would claim a per-row assertion
no check made.

**What blocks it**, one clear line each, contract untouched:

- a failed probe in any stated mode — the package does not behave the way the
  contract says, which is a generator bug or a package change, and neither is
  fixed by converting the claim to unknown;
- an incompleteness finding — a probe observed a callback where the contract
  states none, which makes that negative claim *wrong*, not incomplete;
- a closure `notes` entry on any emitted entrypoint — the summaries were derived
  from a file set the generator itself declines to claim it enumerated, and no
  probe covers the negative claims that file set determines;
- a `kind` claim the report does not record as passing in every mode its export
  is stated for, **when that leaves no entrypoint carrying a certifiable
  export** — there is no sentinel to convert it to, so an unobserved one would
  be certified from nothing, and such a document certifies nothing at all. Where
  an entrypoint with a non-empty export map survives, the unobserved ones are
  refused and omitted instead of blocking;
- a document that would certify nothing for any other reason: no entrypoint at
  all, or nothing but entrypoints with empty export maps. The loader rejects both
  shapes, and the blocker exists so the refusal says what happened instead of
  complaining about document shape;
- a probe report that is missing, describes another package, was written for
  other contract bytes, or predates the `--write` it should have included (its
  passing claims never reached the contract, so all of them would silently
  convert);
- a probe report produced with `--no-discovery`, or one that records no
  discovery state at all — a verified contract certifies every domain its
  summaries omit, discovery is the only automated check that can contradict
  one, and a report that did not run it makes the incompleteness blocker below
  vacuous rather than satisfied. "It probably ran" is not an observation;
- a review plan that is missing or bound to other bytes — its `generation` block
  is the only record of whether each closure could be enumerated;
- a review of this contract that has already recorded a decision or a promotion,
  because verification moves the bytes those decisions were recorded against;
- a promoted document that does not pass `--validate-contract`.

**What does not block:** a refused entrypoint — one the generator refused, or
one verification refuses for an unobserved `kind` claim while another certifying
entrypoint survives (absent from the contract either way, hence already an
explicit uncertifiable result at consumers, and named on the review plan either
way) — an unbindable artifact
(recorded on the review plan; see [Trust boundary](#trust-boundary)), an
undrivable claim (converted, hence demand-sensitively uncertifiable), and a
missing callback `owner` row (fail-closed `SC9012` at the consumer, never
inherited-owner proof).

**It validates before it persists**, the same discipline as `--promote
reviewed`: the promoted document is written to a temporary file in the
contract's own directory, checked with `--validate-contract`, and only then
renamed over the contract, after which the sidecars are written. Verifying a
contract this sidecar already verified — the bytes on disk are still the ones
the promotion produced — prints `already verified`, writes nothing, and exits 0.

**The report, on both paths.** `<contract>.verify.json` is written whether the
promotion happened or not, and the two shapes are told apart by `outcome`
rather than by which counts are zero.

A **refusal** writes `outcome: "refused"` with `blockers.raised` carrying every
line the command printed, `blockers.checked` carrying the same taxonomy the
success path lists, the probe report's own figures, and `contract.before` with
no `after` because nothing was written. Every field that would imply a
promotion is *absent* rather than zeroed: no `evidence`, no `conversions`, no
`probed`, no `summary`. The refusal path used to write nothing at all, which
made the most common outcome the least legible one — stderr was the only record
of why a contract was not promoted, so CI kept a log or kept nothing, and the
ecosystem measurement had to recover the blocker taxonomy by pattern-matching
English sentences. A refusal sidecar is overwritten by the verification that
succeeds later; it is never mistaken for one, because the idempotence check
tests `contract.after`, which a refusal has not got.

A **promotion** writes the blockers checked (with `raised` empty, because this
document was promoted), the probe report consumed and its contract hash, the
identities the result is a function of (generator, probe driver, verifier,
dialect, Solid release, installed version and integrity), the consumed report's
discovery state and its `environment` and `sessions` blocks — so a contract
verified from observations made against a faked `window` is legible as one —
the probed rows that survived, the `staleProbedMarkers` the document carried
that the report did not witness, the `refusedEntrypoints` the promotion left out
with the blocker that refused each one, and — the part that matters — every conversion,
with the claim identity, the value the machine held, the modes it was stated
for, and the reason the probe could not reach it:

```json
{
  "conversions": [
    {
      "entrypoint": ".",
      "export": "needsOptions",
      "field": "callbacks",
      "modes": ["client", "development", "production"],
      "claimed": [{ "parameter": 1, "execution": "inline" }],
      "claims": [
        {
          "claim": "callbacks[1]=inline",
          "reason": "the synthesized call threw: TypeError: Cannot read properties of undefined (reading 'value')"
        }
      ]
    }
  ]
}
```

That record lives outside the contract because schema v1 cannot carry it:
`$defs.unknownClaim` permits the single property `status`, every evidence
definition is `additionalProperties: false`, the loader's unknown-field failure
is the *malformed* path that fails the analysis outright, and `isUnknownClaim`
tests for exactly one key — so a sentinel carrying a `reason` would stop being
recognized as a sentinel by the generator's merge, the review plan, and both
promotions alike. RFC 0002 unresolved question 5 owns whether that is enough.

**Verified is not reviewed.** Certification accepts `verified`, `reviewed`,
`trusted`, and `attested` identically, and `contract check` reports a
project-owned contract as `local` for all of them, so a project cannot today ask
to certify only human-reviewed contracts — RFC 0002 unresolved question 6. What
actually differs is what the tier can *contain*. A machine-verified contract
systematically carries no callback `owner` row, so consumers stay on `SC9012`
for cleanup, flush, and nested-primitive obligations; and every domain the
machine could not confirm is an `SC9005` uncertifiable result exactly where the
surface is touched. A reviewed contract is where those become claims.

**The two compose, in that order.** Verification rewrites the review plan to the
verified bytes, so `contract review` still runs on the promoted document — with
the converted domains now listed as `unknown-sentinel` items a human can resolve
`absent` or by edit, and then `--promote reviewed` on top. Probed rows survive
that promotion untouched.

The rewrite preserves `because`, in both directions. A contract document
carries no generation-time attribution, so re-deriving the plan from the
promoted bytes used to throw away the only record of *why* a claim is
unknown — which proof obligation forced it, where, and how emission decided it
belonged to this export. Every item the rewrite reproduces now inherits the
prior plan's `because` by id (an item's id is derived from what it is about, so
an unchanged question keeps its identity through a promotion), and every
sentinel the verification itself created gets a `because.conversion` mirrored
from the sidecar: the modes the claim was stated for, the claim identities lost,
and the reason each could not be reached. Listing a verified contract does **not** exit 1: the
gate answers "does this contract certify anything yet", and for a verified
document the answer is already yes, so its remaining items are reported as the
optional upgrade. `--promote reviewed` still refuses on every one of them, which
is where the human-decision invariant actually lives. The reverse order is
refused: verification of a contract whose review has recorded anything is a
blocker, and verification of a contract that already carries `reviewed`,
`trusted`, or `attested` evidence would replace a stronger claim with a weaker
one.

**The upgrade path is regenerate, re-probe, re-verify** — there is no transfer.
`--transfer-from` refuses a verified source outright: a verification is a pure
function of the installed artifact, the generator identity, and the probe-driver
identity, so it is *reproduced* rather than carried, and borrowing the human
tier's transfer would launder an observation of one release into a claim about
another. Regenerating in place still snapshots the verified contract to
`<contract>.previous.json` and moves `<contract>.verify.json` and
`<contract>.probe.json` with it, because both are the audit trail for exactly
those bytes; the fresh draft starts with neither, and generation prints the
probe-and-verify pair instead of a transfer command. The cost is probe time per
upgrade, and it is not small — every claim, in every applicable mode, with an
initial and a subsequent call. Caching probe results against `(package, version,
integrity, generator identity, probe driver identity)` is RFC 0002 unresolved
question 9 and is not implemented.

**The trust root is still the generator.** Probes confirm family-(B) claims and
falsify negatives; they never verify a negative. Every omitted effect field in a
machine-verified contract rests on the static soundness of the generator's
fail-closed construction, bounded by the differential harness, the torture
corpus, and the incompleteness path — not eliminated by any of them. RFC 0002
Stage 3 upgrades the closure record from a syntax walk to a compiler
attestation, which attests *which files were read* and still not that the
conclusions drawn from them are right (unresolved question 7).

### Refused entrypoints versus failed generation

An entrypoint the generator cannot certify is refused and omitted, the other
entrypoints are still emitted, stdout appends `; N entrypoint(s) refused and
omitted`, the review plan lists each refusal with its reason, and the command
exits 0. A refused entrypoint is absent from the contract, so a consumer
importing it gets an explicit uncertifiable result rather than a wrong claim.
Refusing one unrepresentable subpath must not cost a package its other twenty.

That exit-0 path is reserved for refusals the generator *decided*: an
unsupported CJS-only target, a missing or escaping export target, an
unsupported export-map shape, conditional branches whose semantics cannot be
reconciled or ordered, a dependency-contract cycle, and the native checker's
own `emit package contract:` fail-closed refusals. Anything else — a bug in
the generator, an unreadable or malformed file, a panicked or
handshake-mismatched native process — proves nothing about the entrypoint,
fails the whole generation, and exits non-zero with the original message.
Recording an unexpected error as "refused and omitted" would turn a run that
used to fail loudly into a silently partial contract. When every entrypoint is
refused, generation fails and names the first real reason.

For a repeatable source-vs-boundary parity check, provision the audited Solid
typings and run:

```sh
make contract-differential
```

The harness analyzes a package implementation as project source, generates its
contract, promotes the generated rows only inside the test to an explicitly
reviewed fixture, and analyzes the equivalent declaration/runtime consumer.
It compares semantic findings while separately asserting callback-path and
owner-requirement rows. This catches behavior lost at the contract boundary;
it does not turn inferred generator output into production evidence.

The lower-level single-project workflow remains useful for packages without a
`package.json#exports` map:

Analyze the package and emit its solved exported function summaries:

```sh
solid-checker --project package/tsconfig.json \
  --emit-contract package/solid-reactivity.json \
  --package-name reactive-package \
  --package-version 1.0.0 \
  --declaration-artifact package/index.d.ts \
  --implementation-artifact package/index.js
```

Load the contract while analyzing a consumer:

```sh
solid-checker --project app/tsconfig.json \
  --contract package/solid-reactivity.json \
  --format json \
  --certify
```

`--contract` is repeatable. Contracts published as
`node_modules/<package>/solid-reactivity.json` are discovered automatically,
including contracts for package subpaths. Schema v1 records an `entrypoints`
map keyed exactly like `package.json#exports` (`"."`, `"./state"`,
`"./server-functions"`, and so on). Explicit contracts override discovered and
bundled contracts. The loader binds an import to its exact entrypoint and
export through Type Facts; it does not fall back to the root entrypoint. When a
package export is followed through a relative project barrel, the loader joins
the binding by the canonical Type Facts `runtimeIdentity`, not by spelling or
filesystem guesses. Conflicting exact summaries produce an explicit
uncertifiable contract result; an empty or unresolved identity remains
fail-closed.

The on-disk format stores each distinct effect summary once. Entrypoints group
export names by summary identifier, and an identical subpath surface can use
`sameAs`:

```json
{
  "schemaVersion": 1,
  "package": { "name": "reactive-package", "version": "1.0.0" },
  "compilerFactsProtocol": 1,
  "summaries": {
    "function": { "kind": "function" },
    "function-1": {
      "kind": "function",
      "callbacks": [{ "parameter": 0, "execution": "tracked" }]
    }
  },
  "entrypoints": {
    ".": {
      "exports": {
        "function": ["createRoot"],
        "function-1": ["createMemo"]
      }
    },
    "./client": { "sameAs": "." }
  },
  "evidence": { "kind": "inferred", "generator": "solid-checker" }
}
```

This normalization is only a wire-format concern. The loader expands it to the
full entrypoint/export model before resolution and analysis, so summaries remain
as expressive as before without repeating `{ "kind": "function" }` hundreds of
times. Summary identifiers are document-local and have no semantic meaning.

Every contract requires a package version and is accepted only when that exact
version is installed. A bundled beta-31 contract therefore cannot silently
certify beta 30, a later beta, or Solid 1.x.

Application developers can also maintain a contract when a package does not
publish one. Put it at:

```text
.solid-checker/contracts/<package>/solid-reactivity.json
```

Scoped names retain their directory structure, for example
`.solid-checker/contracts/@scope/package/solid-reactivity.json`. Project-owned
contracts are discovered automatically and override contracts from
`node_modules`; an explicit `--contract` still has the highest precedence.
The same `--emit-contract` workflow can generate this file when the package
source and a TypeScript project for it are available, or it can be authored
against the contract schema and checked with `--validate-contract`.

### Generating every missing contract at once

```sh
solid-checker contract generate --missing [--project tsconfig.json] [--format text|json]
```

This runs the same coverage report as `contract check` and generates a
project-owned contract for every package it reports as `missing`, from package
root `node_modules/<package>` into
`.solid-checker/contracts/<package>/solid-reactivity.json` (scoped names retain
their directory structure). Both the package roots it reads and the outputs it
writes are anchored at the project directory, exactly as contract discovery
resolves them.

`missing` is the only status it acts on. An `unverified`, `stale`, or `unbound`
package already has a contract on disk that someone owns — a draft mid-review,
one whose remedy is a regeneration *plus* a re-review, or one whose remedy is a
decision about what owns the specifier — so the sweep lists it with the
remedy the report printed and does not touch the file. Statuses that
certify are not listed at all. Because `--missing` describes a set of packages
rather than one, it takes only `--project` and `--format`; `--package-root`,
`--output`, `--entrypoint`, `--conditions`, and `--contract` are single-package
assertions and are rejected rather than applied to every package.

Each package is generated in isolation. A refusal is unchanged — refused
entrypoints are omitted, the package's own line carries `; N entrypoint(s)
refused and omitted`, and the package counts as generated. An outright failure
proves nothing about the next package, so it is recorded and the sweep
continues; the command then exits non-zero and names every failed package.
Text output writes the failure's **first line** to stderr, and the JSON report's
`failed[].reason` carries the **whole original message**, newlines included — a
native panic's useful part is on the lines after the first, and a CI run that
kept only the first had no record of why a package failed. Otherwise it exits 0,
including when nothing was missing.

Every contract this sweep writes is project-owned, and a project-owned contract
sits outside the package by construction — so it is never byte-bound at the
loader, and its review plan always carries a `contract artifact binding` line
saying so. That is the default shape here, not an edge case; see the
[trust boundary](#trust-boundary) section for what the binding can and cannot
express.
The sweep is not a coverage gate: `contract check` is, and it still exits 1 for
the contracts this sweep just wrote, because generated contracts remain
`inferred`. Every generated contract gets its sibling review checklist, and
nothing here promotes evidence.

## Checking contract coverage and freshness

Inspect imported Solid-dependent packages and their contract coverage:

```sh
solid-checker contract check
```

`solid-checker --project app/tsconfig.json --check-contracts` is the same
report; `contract check` accepts the same `--project`, `--format`, and
`--contract` options and defaults to `tsconfig.json` and text output. Each
package is reported as exactly one status:

| Status | Meaning | Certifies |
| --- | --- | --- |
| `bundled` | This checker's own audited contract matches the installed version. | yes |
| `published` | The package ships a contract for its installed version. | yes |
| `local` | A project-owned contract under `.solid-checker/contracts/`. | yes |
| `explicit` | A contract passed with `--contract`. | yes |
| `unverified` | A contract whose evidence is `inferred`; its claims were never reviewed. | no |
| `stale` | A contract that describes a **different artifact** than the one installed — another version, or another npm integrity under the same version. | no |
| `unbound` | A usable contract for the installed version that describes **no import in this project**: every specifier carrying its name resolves somewhere the contract's package is not. See [Which imports a loaded contract describes](#which-imports-a-loaded-contract-describes). | no |
| `missing` | No contract for a package whose manifest depends on or peers with Solid. | no |

Every non-certifying status prints the action that resolves it, and the command
exits with status 1 when any package needs action, so it works as a CI gate.
The JSON format reports, per package, a `remedy` field carrying the same action
and a `detail` field naming the reason when the status alone does not say it
(the two disagreeing versions behind `stale`, the evidence kind behind
`unverified`, the install nothing resolves into behind `unbound`). Both are
omitted for a status that certifies. The report also
carries `missing` (the count of packages needing action) and `stale` (the drift
subset of that count).

### Stale contracts

A contract names the exact package version it was generated and reviewed
against. When the installed version moves — an upgrade, a lockfile refresh, a
different resolution — the contract stops being evidence about the package the
project actually has, and the checker refuses to apply it. This applies to
every tier, `--contract` included, and it does not depend on *how* the project
reaches the package: a contract is classified against the installed manifest
whether the package is imported, re-exported with `export … from`, or not
referenced at all. A package that is not installed has no manifest to disagree
with, so an explicit contract for it still applies.

For a project-owned or published contract, the remedy is to regenerate and
re-review it:

```sh
solid-checker contract generate --package-root node_modules/reactive-package \
  --output .solid-checker/contracts/reactive-package/solid-reactivity.json
```

Regeneration rewrites the contract and its review checklist; the checklist still
has to be reviewed, because generation never promotes inferred claims to
reviewed ones. **Regenerating in place keeps the review it invalidates.** When
`contract generate` is about to overwrite a contract that has a
`<contract>.review-state.json` beside it, it first moves the whole triple —
contract, `.review.json`, `.review-state.json`, and the `.review.md` checklist —
to `<contract>.previous.json` and its matching siblings, then writes the new
triple with no review state of its own, and appends the exact next command to
stdout. Without that move the documented sequence could not be run at all:
`--transfer-from` needs the old contract *and* its review state, and
regenerating in place had already destroyed both. Any older `.previous` set is
replaced — two regenerations with no transfer between them mean the middle
contract was never reviewed. A contract with no review state beside it is not
snapshotted; there is nothing to carry.

So the whole upgrade is three commands, run exactly as written:

```sh
solid-checker contract generate --package-root node_modules/reactive-package \
  --output .solid-checker/contracts/reactive-package/solid-reactivity.json
solid-checker contract review .solid-checker/contracts/reactive-package/solid-reactivity.json \
  --transfer-from .solid-checker/contracts/reactive-package/solid-reactivity.previous.json
solid-checker contract review .solid-checker/contracts/reactive-package/solid-reactivity.json \
  --promote reviewed
```

The transfer carries the previous review's resolutions onto every entrypoint
whose runtime-module closure is byte-identical, so only the diff needs
reviewing — and a republish that changed no implementation byte needs no new
decision at all, which makes the middle command the whole review. For a
*bundled* contract the remedy is different and the report says so: the consumer
does not own that artifact, so the options are to install the version this
checker audited or to upgrade `solid-checker` to a release that audits the
installed one.

Analysis fails closed on the contract without failing the run. The stale
contract is refused — a contract for another version is not weaker evidence, it
is evidence about a different artifact — and the package is reported exactly as
an uncontracted one: an uncertifiable `SC9005 package-contract-incomplete` finding
at the package import, snapshot status `uncertifiable`, and `--certify` exiting
1. The message states which case applies, naming both versions rather than
claiming no contract exists, and the hint carries the same remedy the report
prints.

Refusing the contract without stopping the run is what keeps one upgraded
dependency from blanking out every other finding in the project, which matters
most in an editor. It does not weaken enforcement: the project cannot certify
until the contract is regenerated and reviewed.

A *malformed* contract — unparseable, wrong schema version, wrong package name,
mismatched artifact hashes — still fails the analysis outright. That is a broken
file rather than drift, and no finding can describe a document the loader could
not read.

Missing and unverified contracts take the same path and have always done so.
This behavior is shared by one-shot and retained-daemon checks. Use
`contract check` when only the focused coverage report is needed.

#### Integrity drift under an unchanged version

A version string is not a pin. A republished tarball, an `npm overrides` entry,
and a locally patched install all keep the version the contract names while
replacing the bytes it describes. A contract may therefore also record
`package.integrity` — the npm sha512 integrity of the exact tarball it was
audited against — and every bundled contract does.

When a loaded contract carries `package.integrity` **and** the installed copy's
integrity is recoverable from the project's npm lockfile, a disagreement
refuses the contract exactly as a version disagreement does: status `stale`,
an uncertifiable `SC9005` at the import, the run continues. The message and the
report `detail` name **both integrities** rather than the versions, because the
versions agree — naming them would read as a contradiction. The remedy is to
regenerate and re-review a project-owned contract; for a bundled one it is to
install the exact audited artifact or upgrade `solid-checker`.

The installed integrity is read from `package-lock.json` or
`node_modules/.package-lock.json` (`lockfileVersion` 2 or 3), whose `packages`
map is keyed by *install path* and therefore names the specific installed copy
rather than a package name. Everything else yields **no fact**, and no fact
means the previous behavior — version matching alone:

- no npm lockfile (pnpm, Yarn, or none at all);
- `lockfileVersion` 1, whose tree is keyed by package name and so cannot say
  which installed copy an entry describes under hoisting;
- an entry with no `integrity`: a workspace link, a `file:` dependency, a git
  dependency;
- two lockfiles that disagree about the same installed directory — which one is
  authoritative is exactly the question this cannot answer;
- an unparseable lockfile, which is the project's file and not a malformed
  contract, so it never fails the run.

A contract with no `package.integrity` is unaffected. The residue is real and
deliberate: on a pnpm or Yarn project a contract still binds to nothing but a
version string. See [precision-backlog.md](precision-backlog.md).

Validate contracts and their artifacts without opening a TypeScript project:

```sh
solid-checker --validate-contract package/solid-reactivity.json
```

## Trust boundary

The schema is [solid-reactivity.schema.json](../schema/solid-reactivity.schema.json).
The loader fails closed on:

- unsupported schema or compiler-facts protocol versions;
- unknown JSON fields or malformed summaries;
- missing or unused summary identifiers, duplicate exports, and entrypoint
  alias cycles;
- unsupported effect or evidence kinds;
- imports of entrypoints or exports missing from an otherwise valid contract;
- unsafe artifact paths; and
- declaration or implementation hashes that do not match the files beside the
  contract.

It refuses the contract without failing the run — the stale path — on:

- a contract whose `package.version` is not the installed one; and
- a contract whose `package.integrity` disagrees with the integrity the
  project's npm lockfile records for the installed copy. See
  [Integrity drift under an unchanged version](#integrity-drift-under-an-unchanged-version)
  for exactly when that fact is available and what happens when it is not.

### Which imports a loaded contract describes

A contract describes one installed package, so loading it is only half the
question: the other half is which import specifiers it may be applied to. That
is decided per specifier, by the resolution the compiler itself recorded, in
`PackageContract::for_import`
(`rust/crates/solid-reactive-ir/src/lib.rs`).

The specifier's name is the prefilter and the attested resolution is the
confirmation:

1. **No resolution facts at all.** The analysis was not configured to attest
   identities, and the older name-matched answer stands unchanged. This is the
   WASM adapter without `resolvedImports` in its request — see below.
2. **The specifier is not attested** — the answer did not cover its file, holds
   no row for it, or holds more than one row it could be. The contract is
   refused.
3. **The compiler resolved nothing** for the specifier: the contract applies.
   This is the honest answer for an untyped JavaScript package, which is
   precisely where a contract matters most, and for a specifier typed by an
   ambient `declare module`. Nothing resolved means no *TypeScript-visible*
   claim on the specifier — which is the limit of this clause, and the limit of
   every fact the checker holds. An ambient `declare module` for a package that
   is installed nowhere is accepted here on exactly that basis: the compiler
   could not resolve it, so no shadowing package can be what the contract is
   describing *as far as the compiler can see*. What the runtime loads for that
   specifier is out of reach, and `tsc` says nothing about it either. See the
   residue list in
   [precision-backlog.md](precision-backlog.md#named-residues).
4. **The compiler resolved a file** and the contract was classified against an
   installed directory: the resolved file must lie inside that directory,
   compared on realpaths so a pnpm or workspace-linked install is not a
   mismatch.
5. **The compiler resolved a file** and classification had no installed
   directory — an explicit `--contract` for a package the ancestor walk never
   found: the resolution must have walked into a `node_modules` tree, *and* the
   contract's package name must be the one the resolution recorded, by *either*
   the nearest manifest above the resolved file or the identity the resolver
   itself recorded. Two package identities exist and can disagree, and both are
   accepted, because a published package routinely ships an unnamed
   `{"type":"module"}` manifest beside its output and a subpath resolution
   routinely records no resolver identity.

   The `node_modules` requirement is what keeps this clause off the analyzed
   project's own source, and name equality alone cannot do it. A bare specifier
   that resolves *outside* every install tree is a `paths` or `baseUrl`
   mapping, a package self-name, or a project-reference redirect — the compiler
   records the resolution as `nonRelative` and does not say which — and all
   three name source this project owns. The names can still agree there: a
   monorepo package aliased to its own sources has a root manifest declaring
   the very name its published contract carries, which is the whole of the
   shape pinned by
   `fixtures/reactive-ir/package-contract-uninstalled-name-match`. With no
   install directory to compare against, the resolution having landed in an
   install tree is the only remaining fact that the contract is describing
   installed bytes. The clause still has work to do: a nested or unhoisted
   install (`packages/app/node_modules/pkg` under a root-level tsconfig) is one
   the ancestor walk never classified while the resolution reports it plainly.

What this closes is a false certification, not a missed one: a tsconfig `paths`
entry mapping `"reactive-package"` onto project source while
`node_modules/reactive-package` is installed used to get the installed
package's contract applied to code its author never saw, driving reactive-read,
callback-timing, and owner-requirement conclusions about it. The refusal is
deliberately silent — it produces no finding of its own, and the import becomes
uncertifiable exactly as an unknown package's would.

**A refusal is silent, not invisible.** Two places report it. `SOLID_CHECKER_TIMINGS=1`
adds `contractBindingsBound` and `contractBindingsRefused` — declarations a
contract named and the resolution then confirmed or refused — so a defect in the
span join, in the attestation scope, or in a WASM host's specifier offsets shows
up as a count rather than as contract coverage quietly draining away. And
`solid-checker contract check` reports a contract that binds *no* import in the
project as `unbound` rather than as the tier that supplied it: that command
answers "is my contract coverage complete?", and a contract nothing binds is not
coverage. It counts toward the packages needing action, with a remedy that names
what to look for — a path or baseUrl mapping, or a typings entry pointing
outside the package — rather than anything to do to the contract file. The
analysis path deliberately does **not** raise a finding for the same fact — the
imports go uncertifiable on the rules' own terms — and that split is why
`--check-contracts` now performs the same identity attestation a diagnostic run
does.

Two outcomes are accepted rather than worked around, and both are pinned by
`fixtures/reactive-ir/package-contract-install-shapes`. A package typed through
`@types/<name>` resolves into the `@types` package, which is not the contract's
install, so its contract is refused: reading "`@types/x` describes `x`" out of
the two names is exactly the name-only reasoning this rule removes. And a
refusal does not fall back to a shorter name-matching contract.

The resolutions come from the Type Facts producer's resolved module graph, asked
once per program generation and scoped to the files that carry at least one bare
specifier (`contract_identity_scope` in
`rust/crates/solid-facts-backend/src/lib.rs`). The scope deliberately does not
consult contract discovery: a retained session reuses one generation's facts
across many checks while contracts are re-discovered on every check, so a scope
keyed on today's contracts would answer for a contract that appeared afterwards
by silently omitting its files — name-only binding restored by accident.
`SOLID_CHECKER_TIMINGS=1` reports the operation's cost and coverage as
`importIdentityNs` and the `importIdentityFiles*` counts.

**This is not dialect selection, and deliberately so.** The dialect walk
(`rust/crates/solid-facts-backend/src/dialect.rs`) also keys on `solid-js`, and
it answers a different question about a different object: which Solid version is
*installed* for this project, read from that package's own manifest, before any
program exists — the compiler that produces the facts is the dialect's, so the
answer cannot depend on facts. Identity binding answers where one specifier
resolves. The two agree on the object they both look at (the install), and a
project that maps `"solid-js"` through `paths` onto its own source keeps running
the installed version's catalog while the bundled `solid-js` contract is refused
for that import. That split is correct: the catalog is the vocabulary of the
Solid version installed, and `native_vocabulary_outranks_contract` already gives
those native semantics precedence over the contract, so refusing the contract
removes only the contract-derived summaries. Nothing here changes dialect
selection.

`fixtures/reactive-ir/ported-structure-v2` is that split, live and committed:
its `solid-js` stub sets `"types": "../../solid-js.d.ts"`, so the specifier
resolves *outside* `node_modules/solid-js` and the bundled contract is refused
(`contractBindingsRefused: 1`, `contractBindingsBound: 0`) while every finding
in the fixture is unchanged, because the 2.0 catalog the stub selects is what
those findings rest on. `contract check` on that project reports `solid-js` as
`unbound`, which is the honest answer about the *contract* and says nothing
about the catalog. A published package cannot point `types` outside its own
tarball, so this is a fixture shape rather than one a real install produces.

**The WASM adapter.** `solid-checker-wasm` has no Type Facts session of its own —
the host runs TypeScript and hands the finished tables in — so it cannot ask
where a specifier resolves. `CheckRequest.resolvedImports` is how a host
supplies the answer; see [packages/wasm/README.md](../packages/wasm/README.md).
A request that omits the field binds package contracts by specifier name, which
is what that adapter has always done: a stated limitation of the adapter, not a
weaker analysis of the same request. When the field *is* supplied it is
all-or-nothing per specifier — a file it omits has no answer, and a contract is
refused there rather than falling back to the name.

Two things about a supplied row are checked rather than trusted, because this
interface's failure mode is silence: a row that cannot be joined refuses the
contract exactly as a project with no contract would, so a host mistake would
read as contract coverage varying by file. The spans are UTF-8 **byte** offsets
into the source the same request carries, and the source at the span must read
as the specifier — TypeScript reports positions in UTF-16 code units, so a host
forwarding them unconverted is right for ASCII and silently wrong after the
first non-ASCII character. And `resolvedPath` must be empty exactly when
`resolution` is `unresolved`; an `unresolved` row is *accepted* by clause 3, so
labelling resolutions the host did not perform is the one mistake here that
fails open. Either violation is a hard error naming the row, like an
unrecognized `resolution` value. The native path needs neither check: the same
pinned producer reports the spans and the sources they index.

Artifact hashes use `sha256:<lowercase hex>`. The artifact flags hash exact file
bytes and require each file to be inside the emitted contract's directory.
Artifacts remain optional because they are not always available at emission
time, but they are verified whenever present. The contract itself is SHA-256
hashed when loaded, and that identity is included in the certification package
summary.

`contract generate` emits `artifacts.implementation` whenever schema v1 can
carry the binding honestly: the contract's emitted entrypoints resolve to
exactly one runtime artifact, and that file lies inside the contract's own
directory — the in-package default output. A version string is not a pin,
since republished or locally patched bytes keep the version, so this is what
ties a generated contract to the *entry* artifact it was generated from. The
emitted hash is checked immediately: generation validates the document it wrote,
and the loader verifies every artifact hash it finds.

Three cases cannot be expressed and each carries a `contract artifact binding`
line on the review plan naming why:

- **Several runtime artifacts.** A multi-entrypoint package's entrypoints
  resolve to several implementation files, and schema v1 records one
  `implementation` pair. Hashing one of them would claim byte identity for a
  contract whose other entrypoints describe files nothing pins. Left unbound.
- **An out-of-package output.** A project-owned contract at
  `.solid-checker/contracts/<package>/solid-reactivity.json` is outside the
  package by construction, so its artifact path could only be spelled with
  `..`, which the loader rejects. Left unbound.
- **The entry artifact's runtime-module closure.** The summaries are derived
  from the entry target *and* every runtime module it pulls in, so a barrel
  entry's semantics come from files the single hash does not cover. The hash is
  still emitted — it is real evidence about the entry file — and the review plan
  states that the entry artifact is hashed and how many closure modules are not
  byte-bound. A specifier the walker could not resolve to a file adds a fourth
  line here, reading `closure could not be fully enumerated: <specifier>`: the
  contract is then bound to *less* than its entry artifact, because the hash
  covers bytes whose dependencies nobody enumerated.

`artifacts.declaration` is never generated: this generator analyzes runtime
targets and never resolves the `types` condition, so it has no declaration file
whose bytes it could honestly claim to have read. See
[precision-backlog.md](precision-backlog.md) for the residue.

The bundled Solid 2 artifacts also ship
`pkg/contracts/bundled/runtime-lock.json`. It records the resolved version and
npm integrity for every dependency and peer edge in the audited
`solid-js`/`@solidjs/web` runtime closure. Conformance checks the installed
manifests, resolved versions, and integrities against that lock; the
`^2.0.0-rc.0` declaration for `@solidjs/signals` therefore cannot drift without
failing the gate.

Changes under `rust/crates/solid-reactive-ir/` run the bounded package-contract
torture corpus in `.github/workflows/contract-corpus.yml`. The corpus covers
runtime-mutated namespaces, conditional semantic branches, getter-backed
exports, deep re-export barrels, and declaration/runtime disagreement. Its
checked-in expected outputs are reviewed like snapshots: an unexplained drift
fails the engine-change gate, and the runner never updates those pins.

Evidence is enforced, not decorative. Contracts emitted by this CLI use
`inferred`; consumers report them as `unverified`, and their summaries are not
inserted into Reactive IR at all. They therefore cannot prove a violation,
suppress an obligation, or certify a consumer. Certification accepts
`verified`, `reviewed`, `trusted`, and `attested` contracts. Legacy `generated`
remains parseable but is also unverified.

Schema-v1 contracts may also put `evidence` on an export summary, reactive-read
row, callback row, or recursive return row. Row evidence is one of `inferred`,
`probed`, `reviewed`, or `inherited-from`; probed rows record `modes` and a
positive `calls` count, while inherited rows record the exact `package` and
`version`. Contracts without row evidence retain the contract-level behavior.
When row evidence is present, certification additionally rejects any inferred
row so a promoted contract cannot hide an uncertified claim inside a verified
document.

Promote an inferred contract only after checking it against the exact package
artifacts and reviewing every unresolved behavior. `verified` means mechanical
artifact/surface/behavior checks passed and is written by
[`contract verify`](#machine-verified-contracts), which takes no decision and
converts every claim it could not confirm to the unknown sentinel first;
`reviewed` records an explicit human review; `trusted` is an out-of-band trust
decision and `attested` is reserved for a verifier-produced release identity,
and no command here writes either.

### Promoting a reviewed contract

Run [`contract probe --write`](#probing-a-generated-contract) first if you
intend to probe at all: it moves the contract's bytes, and once a review has
recorded a decision the write refuses rather than move them underneath it.

Promotion is a recorded sequence of per-item decisions, not a hand edit of the
JSON:

```sh
solid-checker contract review .solid-checker/contracts/reactive-package/solid-reactivity.json
solid-checker contract review <contract> --resolve <id>=absent --note "audited against 1.4.2"
solid-checker contract review <contract> --promote reviewed
```

The command takes the contract or the directory containing it, and it works only
on a contract `contract generate` wrote: it resolves the machine-readable plan
beside the contract, refuses when there is none, and refuses when the plan is
not bound to those exact bytes. A hand-authored contract has no plan, so it
stays on the manual path — author it, check it with `--validate-contract`, and
set its evidence by hand.

With no options it lists every plan item as `[resolved]`, `[open]` or `[stale]`
and exits 1 while any item is open or stale or any unknown claim remains, so it
is a CI and publish gate. `--resolve ID=DECISION` (repeatable) and
`--answers FILE` (a JSON `{id: decision}` map) record decisions into
`<contract>.review-state.json`; `--note` accompanies a single `--resolve`. There
is no interactive mode.

Every argument is parsed and checked before anything is written, so a bad flag
in the last position cannot leave a state file written by an earlier one. An
empty value for `--transfer-from`, `--promote`, `--answers`, `--note`, or
`--resolve` is an error naming the flag rather than a silently disabled option;
two `--resolve` arguments for the same id in one invocation are an error rather
than last-wins; and a `<contract>.review-state.json` this command cannot read —
`resolutions` that is not an object, or an entry with no string `decision` — is
refused rather than replaced with an empty one.

Three decisions, and the items that accept them:

- `confirm` -- the generated claim is correct as generated. Valid for every
  kind except an unknown claim: unknown is not evidence, and confirming a
  marker would promote something that certifies nothing. It is the *only*
  decision a `generated export summary` item takes, because that item is raised
  for as long as the export exists.
- `absent` -- **certify the negative**, explicitly. For an unknown claim, the
  sentinel field is deleted at promotion. For a function export with no
  callback row nothing is written, because the omission already *is* the claim
  -- which is exactly why it has to be said out loud. This is the dangerous
  edit the command exists to make deliberate: deleting a `callbacks` sentinel
  by hand certifies "never invokes a caller-supplied callback", and nothing
  about a raw JSON edit distinguishes that claim from "not decided yet".
- `resolved-by-edit` -- the reviewer edited the contract to carry the audited
  value. Accepted only once the contract's own bytes no longer raise the item,
  and only for kinds a contract can witness; a refused entrypoint, a legacy
  root field, and an artifact binding are facts about generation that no edit
  to the document can answer.

Each resolution records the sha256 of the contract bytes it was made against.
Editing the contract makes every earlier resolution **stale**, and stale counts
as open: a review of other bytes is not a review of these. So an edit obliges a
re-review of everything already decided, and each step can be its own
invocation — record what the generated document already answers, edit, record
`resolved-by-edit`, re-make the decisions the edit made stale, promote. The one
ordering constraint is that at least one decision must be recorded before the
first edit; see the plan-binding paragraph above.

`--promote reviewed` refuses -- one clear line each, contract untouched --
while any item is open, any resolution is stale, any unknown claim in the
contract is undecided, or the contract raises a question the plan does not
list. On success it deletes the fields certified absent, drops the `inferred`
row markers the review resolved, and sets the contract's `evidence` to
`reviewed`.

**It validates before it persists.** The promoted document is written to a
temporary file in the contract's own directory, checked with
`--validate-contract`, and only then renamed over the contract, after which the
review state is written. A document the loader rejects therefore leaves the
contract *and* the review state byte-untouched and exits 1; writing first and
validating afterwards left `evidence: reviewed` and a `promoted` state on disk
for a document the loader refuses, and the next listing reported that as a
completed review and exited 0.

**Promotion is idempotent.** Running `--promote reviewed` again on a contract
this state already promoted — the bytes on disk are still the ones the promotion
produced, and the document says `reviewed` — prints `already promoted`, writes
nothing, and exits 0. Dropping the sentinels a review certified absent turns
those exports into ones with no callback row, which the plan written before the
deletion does not list, so a second run used to refuse and advise regenerating a
contract that was already finished.

Dropping the row markers is what makes promotion mean anything: certification rejects any
inferred row inside a promoted contract, while a row with no evidence of its
own inherits the document's, and writing `reviewed` onto each row instead would
claim a per-row human assertion nobody made. `probed` and `inherited-from` rows
are left exactly as they are. No entrypoint and no export ever leaves the
document -- sentinel deletion and the evidence change are the only mutations.
The command never writes `verified`, `trusted`, or `attested`, which mean a
mechanical check, an out-of-band trust decision, and a verifier-produced
release identity. `verified` has a command of its own —
[`contract verify`](#machine-verified-contracts) — and a review can be recorded
*on top of* one: the machine's converted domains arrive as ordinary
`unknown-sentinel` items, and promoting to `reviewed` leaves the probed rows the
machine earned exactly as they are.

### Transferring a review to a regenerated contract

A contract binds to one artifact, so every upstream release turns a reviewed
contract into a [stale](#stale-contracts) one. Re-reviewing the whole package on
each publish is what makes a reviewed corpus rot with ecosystem velocity;
`--transfer-from` makes an upgrade cost a review of the *diff*:

```sh
solid-checker contract generate --package-root node_modules/reactive-package \
  --output .solid-checker/contracts/reactive-package/solid-reactivity.json
solid-checker contract review <new-contract> --transfer-from <new-contract:.previous.json>
solid-checker contract review <new-contract> --resolve <id>=absent   # what is left
solid-checker contract review <new-contract> --promote reviewed
```

Regenerating in place is what produces `<contract>.previous.json` and its
siblings; see [stale contracts](#stale-contracts) for that move. `--transfer-from`
takes the snapshot path exactly as it takes any other contract path — except a
[machine-verified](#machine-verified-contracts) one, which it refuses: a
verification is reproduced by re-probing the new artifact, never transferred.

It reads the old contract, its `<contract>.review-state.json` (**required** — a
contract with no recorded review has no reviewed conclusion to carry), the new
contract, and the new `<contract>.review.json`. It writes only the new review
state; the old contract, the old review state, and the new contract are
byte-untouched, and running it twice changes nothing — a second run recomputes
the same conclusions against the same bytes and rewrites nothing, and a transfer
onto a review that has already been promoted is refused outright rather than
quietly clearing the promotion.

**The granularity is the entrypoint and the rule is byte identity.** The old
review state carries a copy of the `generation.entrypoints` block recorded when
the resolutions were made — the exact runtime modules the reviewer resolved
against — and the new review plan carries the block the new contract was derived
from. Two preconditions apply to the transfer as a whole: the two contracts must
record the same `compilerFactsProtocol`, and where both plans name the generator
that wrote them, it must be the same one — different code produced the summaries
and the closure enumeration behind them, so byte-identical inputs no longer imply
an equivalent review. The old review state's own `contract` field must also hash-
match the old contract's bytes; a state describing some other document has no
conclusion about this one.

Then, per entrypoint, all of these must hold:

- both blocks record a closure for it, and neither record carries a `notes`
  entry (a note is an omission: a record that is not attested, a module the
  program opened that the seed did not name, a module the program opened that the
  record's scope excludes, or a module whose bytes were unreadable, and an
  incomplete record establishes nothing). A `runtimeNotes` entry does **not**
  block here — the record it sits beside is complete, and what it names is
  unbounded in both generations equally. Both halves of that rule are pinned:
  `scripts/contract-review.test.mjs` drives the comparison directly, and
  `scripts/contract-verify.test.mjs` pins that the same `runtimeNotes` entry still
  refuses the promotion;
- the `targets` lists are identical;
- the `modules` lists are identical — the same module paths, each with the same
  sha256;
- the two contracts' expanded export summaries for the entrypoint agree, once
  the previous review's own mutations are applied to both sides. Deterministic
  generation makes this follow from the closure identity, so it is a check
  against a generator that changed between the two runs rather than a check
  against the package.

The last point needs the projection because a *promoted* old contract is not the
document generation wrote: promotion deleted the claims certified `absent` and
dropped the `inferred` row markers. Both sides therefore get the same treatment
— delete the fields the old review resolved `absent`, drop `inferred` markers —
before they are compared, which is idempotent and so compares an unpromoted old
contract just as well. `probed` and `inherited-from` markers survive it, so a
row that changed which package it was inherited from still blocks the transfer.

For each item of the new plan whose entrypoint transfers, a resolution with the
same id in the old review state is carried over with its decision and its note,
provided that resolution was not itself stale there. **An item's rendered `text`
must also be byte-identical on both sides.** An id is derived from what an item
is *about* — its kind and its `{entrypoint, export, field}` target — and two
items about the same thing can still ask different questions: a legacy root
whose `module` build is unchanged while the manifest newly adds a divergent
`main` raises the same id with different text, and inherited the previous
`confirm` until this condition existed. The old review state therefore records
the text the reviewer saw beside each decision, because the plan file that
showed it has been overwritten by the regeneration.

The written resolution records `transferred: {from, at}` — the sha256 of the old
contract and the original decision's timestamp — beside the new contract's own
hash and the item text, so the audit trail says which conclusions were reached
about which bytes and about which question. A transferred `absent` on an unknown
claim deletes the sentinel at promotion exactly as a locally recorded one does.
A transferred `resolved-by-edit` passes the standard acceptance check against the
*new* bytes: if the new contract still raises the item, the conclusion does not
transfer.

Two item kinds are not about one entrypoint and have their own rules:

- `legacy-root-field` additionally requires the plan's recorded legacy root —
  the manifest field, the artifact it resolved to, and any divergent `main` —
  to be equal where both plans carry it.
- `artifact-binding` transfers only when **every** entrypoint present in either
  contract satisfies the full byte rule, and the text is identical. It is about
  the document as a whole, so nothing less than "nothing the binding could be
  about changed" will do. Making it transferable at all is what re-enables the
  fast path for project-owned contracts, which always carry such an item and
  therefore never had one before.

**What never transfers**, and stays open:

- every item of an entrypoint that fails any condition above;
- every item, when the two contracts disagree about `compilerFactsProtocol` or
  the two plans name different generators;
- an item with no prior resolution, or whose prior resolution is stale in the
  old review state;
- an item whose text differs from the one that was resolved, or whose prior
  resolution records no text;
- `refused-entrypoint` and `no-export-summary` items — both describe an
  entrypoint the contract does not summarize, so there is no closure record to
  witness that it is the same situation;
- a `resolved-by-edit` conclusion the new contract still raises.

The command prints one line per transferred item, then one line per
`(entrypoint, reason)` pair with the count of items that stayed open for it, then
the transferred/open totals, then the standard listing. It refuses outright —
nothing written — when the old review state is missing or describes other bytes,
when the two contracts name different packages (differing *versions* are the
point), when the new review is already promoted (there is nothing to transfer
onto a completed review), or when the new review state already records
resolutions against other bytes than the ones being transferred onto, which
means a review of this contract is already under way: transfer is the first step
of a re-review, and merging it into decisions already taken would leave the
audit trail unable to say which is which. Delete that review state and transfer
first, or finish the review without `--transfer-from`.

**The version-bump fast path.** When a package is republished with an unchanged
implementation — a version bump, a metadata-only release — every entrypoint's
closure is byte-identical, every plan item renders the same text, the two plans
name the same generator and protocol, and so every item transfers, including the
`artifact-binding` line a project-owned contract always carries. A single
`--promote reviewed` then succeeds with no new human decision.

## Effect summaries

The schema records:

- direct reactive accessor and store-path reads;
- accessor and store returns, including factory-to-factory propagation;
- tuple slots and named object properties containing reactive returns;
- inline, tracked, and deferred callback parameters;
- Promise and async-iterable behavior;
- inert exported values; and
- inferred, verified, reviewed, trusted, or attested evidence.

Omitting an effect field is a reviewed negative claim: the behavior is known
not to occur. When a producer can prove other parts of an export but cannot
complete one claim domain, schema version 1 uses an explicit sentinel in that
existing field:

```json
{
  "kind": "function",
  "callbacks": { "status": "unknown" }
}
```

The same sentinel is valid for `reactiveReads`, `returns`,
`ownerRequirements`, and `asyncBehavior`. It is intentionally not a sibling
`unknownClaims` list: old readers could ignore a new sibling and misread the
omitted behavior as proven absent. The sentinel has the wrong JSON type for
each old field, so old schema-v1 readers reject it and fail closed.

Unknown is not evidence and cannot be promoted. Review resolves the marker by
replacing it with the audited value, or by deleting the field to certify that
the behavior is absent. `contract review` makes the choice explicit --
`resolved-by-edit` for the first, `absent` for the second -- and refuses to
promote a contract whose marker is neither; see [Promoting a reviewed
contract](#promoting-a-reviewed-contract). Inferred documents retain the
existing SC9005 trust ceiling regardless of whether they contain markers.

Generation covers function declarations, exported arrows, overloads, nested
generics, async functions, multiple const declarations, classes, re-exports,
aliases, and subpath imports. Consumers support named imports and local aliases.

Packages without an `exports` map can still be generated when they expose one
exact legacy ESM root: `module`, an ESM-safe `main` (`.mjs`/`.mts` or a runtime
file under `type: module`), or an unambiguous ESM `index` fallback. `module` is
the bundler's ESM entry and `main` is what Node's own resolver loads; when they
name different artifacts the contract describes only the analyzable ESM one.
Schema v1 has no condition that distinguishes those fields, and refusing the
package would reject every legacy dual package including the common case where
`main` is just the CJS transpile of the same source, so the review plan records
which field the root came from and leaves the equivalence judgment to review. These forms
produce only the `.` entrypoint. CJS `main` targets, missing legacy targets,
absolute/out-of-package paths, and ambiguous fallbacks remain unsupported;
generation never parses CJS as if it were an ESM runtime artifact.
Calls in compiler-tracked JSX retain their tracked status; calls in ordinary
function bodies produce `strict-read-untracked` findings.

Generated `ownerRequirements` use the compiler's canonical symbol identity for
aliases and re-exports, plus exact AST function identity for anonymous default
exports that have no name symbol. An operation is assigned only to its
immediate containing function body. A same-named declaration or a nested
closure inside an exported factory cannot lend its owner requirement to that
export.

Structured returns are an additive part of `schemaVersion: 1`. A tuple uses an
`elements` array (with `null` for an uncontracted slot), while an object uses a
`properties` map. Leaves retain the existing `accessor` or `store-path` shape.
Consumers recognize those leaves through array/object destructuring and direct
object member access. An `argument` return identifies a parameter whose actual
value is returned unchanged; consumers instantiate it at each call, so generic
identity and invariant wrappers preserve nested tuple/object reactivity without
inventing a new schema version.

A `callback-result` return instead identifies a callback parameter whose
*invocation result* is returned. Consumers resolve the exact local callback at
the call site and preserve its accessor/store/tuple/object result. An external,
conditional, or otherwise unresolved callback stays unknown; the relation does
not turn a generic `T` into a guessed reactive kind. This is the shape used by
`@solid-primitives/rootless`' `createSubRoot` and `createBranch`.

A shorthand property (`{ pathname }`) writes one identifier where a key and a
value both stand, and TypeScript answers a symbol query at that span with the
*property's* symbol, never the value binding's. The value is identified instead
by the binder's resolution of that exact reference, carried on the object
property fact as `shorthandBinding`. That is scope-exact: a same-spelled
binding in a sibling block declares a different symbol at a different span, so
it can neither be chosen nor make the visible declaration ambiguous. A
shorthand the binder resolves to a relative named/default import is followed
through exact project-local ESM exports (including re-export chains) before a
reactive leaf is claimed. Ambiguous relative targets, bare/path-mapped imports,
namespace imports, globals, and unresolved cycles yield no structured property;
the generator never chooses a same-spelled declaration or filesystem candidate
as a substitute for exact resolution.

When an exported parameter escapes through an uncontracted external call whose
execution semantics are unknown, generation preserves the other proven claim
domains and emits `callbacks: { "status": "unknown" }` for that exact export.
It never emits an empty, falsely inert callback summary. This includes a callee
with no resolvable identity at all -- `list.map(fn)` where `list` is one of the
exported function's own parameters, and therefore `any` in a published
JavaScript runtime artifact. Such a call is not dropped from the analysis: any
argument that is a parameter of the enclosing exported function, and whose own
syntax does not already prove it inert, opens the unknown callback claim. A
consumer then demands that uncertainty only where a potentially callable
argument is actually supplied -- `slice(list, 0, 2)` stays clean, and a read
inside a callback whose timing is unknown stays uncertifiable rather than
becoming a proven untracked read. Local calls are
summarized transitively, and forwarding into known Solid callback slots records
the corresponding tracked, deferred, or inline execution.

**A local callee's summary is only inheritable where it accounts for the
parameter.** Summarizing a local call transitively lets the caller inherit the
callee's callback answer, and an *empty* answer is the negative claim "never
invoked". That inheritance breaks the moment the callee merely retains the
value: `createComputation(fn, init) { const c = { fn, value: init, … }; return
c; }` — solid-js 1.9.14's — calls nothing, so its summary is empty, so
`createMemo`, `createEffect`, `children`, `createSelector`, `createDeferred`,
`createRenderEffect` and `createComputed` each published no callbacks row at
all, and `contract probe`'s discovery pass contradicted every one of them.
Generation therefore tracks *retention* per parameter and emits the unknown
sentinel for the declaring export, propagating it along the same forwarding
edges the callback rows travel.

Retention is a closed list of positions, never "every reference the analysis
did not recognize". The difference is the whole precision budget: a published
runtime artifact is dense with references that only *observe* a parameter —
`typeof value === "string"`, `prev && …`, `for (const key in props)`,
`value[HREF]`, `node[name] = value`, a reassignment of the parameter itself —
and treating those as escapes converts a third of a DOM package's exports to
sentinels while proving nothing. The retaining positions are an object-literal
property value (`{ fn }`), an assignment value (`source = pSource`) whose
target is not a member chain rooted at one of the caller's own parameters, and
a computed read of a rest parameter (`sources[index]`, whose slot no
`callbacks` row can name). `fixtures/package-contracts/retained-callback-parameter`
pins both halves.

References are resolved through the *binder's* own answer for that exact
reference, not through a compiler entity: TypeScript answers a symbol query at
a shorthand property span (`{ fn }`) with the property's symbol rather than the
value binding's, and an entity exists only where some demand asked for one.

Consumers keep an unknown callback demand-sensitive. Passing a potentially
callable value produces an SC9005 uncertifiable finding through the existing
per-export contract path; a call with no callable argument does not. Unknown
read, return, owner-requirement, and async domains open that same per-export
obligation while their known sibling claims remain available.

`execution: "inline"` is a promise about the **export**: the callback is
invoked before the exported call returns. So the row is written only where the
invocation is proven at that granularity — a `parameter(...)` call in the body
of the function that declares the parameter, a resolved runtime position whose
argument behavior is known (`Array.forEach`, `setTimeout`, a contract row), or
a Solid callback slot. A call written behind a closure boundary the analysis
cannot schedule proves nothing about the export: `f(props, cb) { helper(props,
() => cb(1)) }` may invoke `cb` later, once, or never, and
`f(cb) { return { g: () => cb(1) } }` does not invoke it at all. Both used to
publish `inline` — the lexical execution role of a capitalized function's body
is "rendering", which maps to inline, and a direct callee is a property of the
call rather than of its schedule. Both now write no row and open the
unknown-callback sentinel instead. `fixtures/package-contracts/callback-execution-boundary`
pins the three shapes; the boundary is read off the AST, because an
expression-bodied arrow is a function boundary the summary-node universe does
not always carry.

### Which exports an unresolved obligation belongs to

Every obligation is attributed to the narrowest affected claim domains and to
exactly the exports it can reach. Attribution is a ladder, tried in order; each
rung above the last resolves an exact identity, never a name-text match.

1. **`joined`** — the innermost function whose body contains the obligation is
   itself an export of this entrypoint, by Type Facts runtime identity or by
   canonical symbol. An `export { Inner as Root }` alias and a cross-file
   re-export both resolve here, and `export { Panel, Panel as Root }` resolves
   to *both* names. There is no name-text join: matching
   `exports[local_name]` attributed an obligation inside a private `Render` to
   an unrelated exported `Render`, and stopped at the first name of an aliased
   pair. It survives only in the whole-project mode with no entry file, where
   `exports` is keyed by the project-wide export name and no identity channel
   exists at all.

   A declaration is named the way the IR names one
   (`solid_reactive_ir::function_binding_name`), so `export const X = () => {}`
   — which has neither a function name nor a method name — resolves like a
   declaration. Reading only those two fields made every arrow-bound export
   invisible to every rung below.
2. **`enclosing-chain`** — no, but an *outer* function on the enclosing chain
   is. An obligation inside an anonymous callback, a named local helper, or a
   method belongs to the exported function that lexically contains it.
3. **`identity-widening`** — the obligation's own location is a declaration
   rather than a position inside one; a missing contract export is filed at the
   import binding, which no function body contains. Every reference to that
   exact symbol is resolved through rungs 1 and 2, and the exports those
   references sit in are marked.
4. **`reachability`** — the obligation is inside a private helper the public
   surface never names, or is filed at a helper's own declaration span. The
   call graph (`solid-reactive-ir`) enumerates the project functions that can
   transitively enter it, and only the exports among them are marked. The
   enumeration is used only when it is *complete*: a function on the path that
   is entered by something the graph cannot enumerate — a module-level call, or
   a function value escaping into an unresolvable callee — makes it incomplete,
   and attribution falls through.

   Escape is decided on the *export specifier*, never on containment in an
   `ExportNamedDeclaration`'s span, which covers the declaration's whole body:
   `apply(Panel, …)` and `return Panel` written inside an exported function are
   escapes, and reading them as export surface left the enumeration
   incomplete-but-trusted. A reaching function the ladder cannot name also
   makes the answer unavailable rather than empty — "I cannot tell what this
   function is" is not "it is not an export".

   A rendered tag is not an escape: rendering a component invokes it, so the
   call graph enumerates the tag as a call site (`all_function_call_sites`)
   whenever the tag name resolves to exactly one project function, and the
   escape test accepts that reference because the edge exists — not because the
   reference is a tag. Both spellings are covered: `<Panel/>`, and
   `<Panel></Panel>` (with or without children), whose closing tag is a second
   TypeScript reference to the same symbol. The closing name span
   (`JsxElementFact::closing_name`) rides on the edge the opening tag's
   resolution created — one render is one call site, and a closing tag with no
   resolved opening tag has no edge to ride — so the escape test accounts for
   both spans without the call graph gaining a second caller.

   What still stays an escape: a tag that resolves to nothing; a dotted tag,
   where the edge *is* emitted with the whole `ns.Panel` name span as its callee
   but the reference the escape test walks is the `Panel` property inside that
   name, and the test is byte-exact span membership; and a component handed to
   something else as a value (`<Wrap child={Panel}/>`).
   `escaping-private-helper` pins every one of these, one entrypoint per claim.
5. **`fallback-all`** — nothing identified the obligation's containing function
   at all. Every function export of the entrypoint is marked. This rung is
   fail-closed and still reachable; it is not the ordinary case.

Schema v1's `unknownClaim` carries only `status`, so the rung that answered
cannot be recorded in the contract. It is recorded instead on the matching
`unknown-sentinel` item of `<contract>.review.json`, under `because`, together
with the obligation kind and the file and byte range it sits at — none of which
is otherwise recoverable from the contract document. The native emitter names
each decision on one stable stderr line and the generator attaches it; the
markers never reach human-visible output.

A decision that marks *nothing* — reachability proving that no export of the
entrypoint can reach the obligation — has no `unknown-sentinel` item to carry
it, and leaves the contract byte-identical to one produced by an analyzer that
never saw the obligation. It is reported all the same, as a review-plan note
under **contract artifact binding**, naming the obligation, its location, and
the rung that narrowed it. Silence there was how a truncated reach enumeration
looked from the outside.

Callback rows can also describe the runtime arguments supplied to the callback:

```json
{
  "parameter": 0,
  "execution": "inline",
  "arguments": [null, { "kind": "accessor", "label": "item" }]
}
```

The list reuses the bounded structured-return vocabulary; `null` means an
ordinary or unmodeled value at that position. A consumer materializes an
accessor only on the corresponding parameter of the exact callback function.
This covers mapping helpers that hand a fresh accessor to user code without
misclassifying the handoff as a read performed by the helper.

Because that is the only shape a consumer materializes, every other
schema-valid shape is demand-sensitive rather than silently dropped. A call
site whose callback argument is not an inline function literal — a function
passed by name — and a descriptor whose kind is not `accessor` landing on a
parameter the literal does declare both produce an SC9005 uncertifiable
finding at the argument, through the same per-export contract path as an
unknown claim. A descriptor beyond the literal's declared parameters is not a
gap only when the literal is a *restless arrow*: an arrow has no `arguments`
object, and with no rest parameter there is no binding in its body that could
name the described argument, so nothing it does can depend on it. A non-arrow
function expression can read the slot as `arguments[N]`, and a rest parameter —
which is not one of the declared parameters — absorbs every argument from its
index onward; both therefore fail closed. An inline literal carrying only
`accessor` descriptors keeps its precise, silent behavior.

An exported function that directly invokes a member through one of its own
parameters is represented by a parameter-attributed reactive read:

```json
{ "kind": "parameter-member", "parameter": 0 }
```

This row records receiver provenance, not a method name or a declaration-file
promise. Generation derives it from the runtime artifact's exact parameter
symbol and direct member receiver, and propagates it through local wrappers.
At each consumer call site, the argument is classified independently: a proven
reactive store/path contributes a store-path read in the call's execution role,
and any other value remains SC9012 `reactive-dispatch-unresolved` unless it is
proven not to be a reactive source. A Solid store is a proxy typed as the object
it wraps, so a declared type never proves that negative; two things do. An
inline literal -- primitive, array, object, or nullish -- was created at the
call site and `createStore` never produced it. An analyzed local binding
initialized by a resolved standard-library call, such as
`document.createElement("button")`, has a platform origin that cannot return a
store. A value whose origin the project cannot see -- a parameter, a prop, an
import, a bare `declare const` -- stays unproven and keeps its obligation. Thus `drop(storeArray)` carries a read while
`drop([1, 2, 3])` does not. Callback invocation remains owned by `callbacks`;
it is not encoded as a parameter-member read. The proof stops at the direct
receiver/wrapper boundary: values stored for later, spread through opaque
objects, or selected by argument identity remain uncertifiable.

Because that row carries the same provenance the project-side obligation
records, emission may discharge the obligation instead of marking a claim
unknown — but only for the exports that actually publish the row. The
provenance does not survive a hop: an export calling `helper(props.client)`
forwards a member of *its* parameter, which re-establishes no parameter of its
own, so it publishes no row and a consumer of that export is told nothing. The
question is therefore asked of the exports the attribution ladder resolves the
obligation to, and the discharge holds only when every one of them publishes
the row (`fixtures/package-contracts/parameter-member-forwarded`). Discharging
on the obligation's `analysisContext` alone published a certified negative for
every export above the helper.

Parameter-attributed writes are not part of schema version 1. A setter or
mutable capability has operation and ownership semantics that a generic
"writes parameter N" row would erase; those cases continue to require an exact
project implementation or a future, separately designed contract claim.

The native generator also recognizes a narrow set of ECMAScript and Web API
runtime positions from the producer's exact standard-library declaration
identity (`qualifiedName` plus standard-library provenance), never from a
spelled API name alone. The audited positions include conversion and factory
value arguments, `Array.from` and typed-array mappers, string replacement
callbacks, observer/geolocation/scheduler callbacks, and collection retention.
This includes collection constructors and insertion methods whose arguments are
values rather than callbacks.
Same-named project declarations remain unknown and fail closed.

Schema-v1 callback entries may additionally carry an exact `owner` value:
`inherited`, `created`, `unowned`, `conditional`, or `leaf`. This lets a
reviewed package contract preserve the owner capability needed by consumer
owner, cleanup, and leaf-operation analysis. The field is optional for
backward compatibility: a missing owner row describes timing only and never
becomes inherited-owner proof. Generators therefore put callback owner rows on
their review checklist rather than guessing them. A reviewed leaf row can
preserve the fact that cleanup, flush, and nested primitive creation are
forbidden in a Solid leaf owner such as `onSettled`; an unreviewed or missing
row remains SC9012 and fail-closed.

Runtime selection is explicit at the native CLI (`--runtime-target`,
`--runtime-build`, `--rendering`, repeated `--runtime-condition`, and
`--framework-transform`) and in the ESLint adapter's
`settings.solidChecker.runtime`. The selected target, build, rendering mode,
conditions, and transforms participate in one-shot, daemon, and adapter cache
identity. A conditional entrypoint or variant is consumed only when exactly
one compatible summary is selected; incomplete, contradictory, or ambiguous
conditions remain uncertifiable. Contradictory target, build, or rendering
conditions are rejected at configuration validation. Explicit CSR/SSR selects the
rendering premise but does not prove request-dependent post-flush timing.

Local deferred-flow proofs are structural rather than name-based. A function
installed on an object is considered deferred only when that object is
caller-owned or returned. A callable constructor parameter is considered
retained only when it is a TypeScript parameter property on an object passed to
an exact compiler-resolved retaining runtime position, such as a Proxy handler.

## Adding a package to a dialect

This section is for maintainers of this repository, adding a package that a
dialect models directly. Application developers generating a contract for a
dependency want [Checking contract coverage and
freshness](#checking-contract-coverage-and-freshness) instead.

### The generate/check model

Dialect contract artifacts are **derived from a declaration plus an installed
package**, and every one of them is checked by regenerating it and comparing:

```sh
make contracts        # write the artifacts
make contracts-check  # regenerate into memory and fail on any difference
```

`make contracts-check` runs in CI's `rust-engine` job on every push and pull
request, after installing the exact pinned releases. A checked-in artifact that
no longer matches what the generator produces from the pinned package is a
failure, not something the next run quietly fixes. Adding a package therefore
means adding its declaration; the artifacts follow from it, and the gate keeps
them honest.

**Only half of a contract is derived, and the halves are checked differently.**
The *export set* is a syntactic fact read from the package's declarations with
the same parser the checker runs on user code, following `export *` and
`export { x } from` chains — so drift in it is caught mechanically. The
*reactive semantics* — whether a function opens a root, establishes an owner,
returns a live store or a snapshot — cannot be derived from a signature and are
hand-authored tables inside `solid-contract-gen`, each carrying its evidence.
`--check` proves the artifact matches the tables; it cannot prove the tables
match the runtime. That is what the runtime probes below are for, and why a
version bump is a re-audit rather than a regeneration.

### Declaring the package

Add one entry to the `contracts` array of `rust/dialects/<id>/dialect.json`:

```json
{
  "package": "@solidjs/web",
  "packagePathEnv": "SOLID_V2_SOLIDJS_WEB_PACKAGE",
  "defaultPackagePath": "node_modules/@solidjs/web",
  "generatorTarget": "solid-v2/solidjs-web",
  "reviewContract": "rust/crates/solid-dialect/contracts/solid-v2/solidjs-web.json",
  "exportsIndex": "rust/crates/solid-dialect/src/exports/solid_v2_solidjs_web.rs",
  "bundledContract": "pkg/contracts/bundled/solid-v2/solidjs-web.json",
  "probeRuntime": true
}
```

Every field except `probeRuntime`, `composeScript`, and `composeInputs` is
required, `generatorTarget` must start with `<id>/`, and no two entries may
share a `generatorTarget` or declare the same package twice.

A contract that is **reviewed against a package rather than derived from it**
declares `"generated": false` and carries only `package` and `bundledContract`:

```json
{
  "package": "@solid-primitives/scheduled",
  "bundledContract": "pkg/contracts/bundled/solid-v1/solid-primitives-scheduled.json",
  "generated": false
}
```

There is nothing for `make contracts` to regenerate from, so it skips these
entries. Supplying any generator field alongside `"generated": false` is an
error rather than being ignored: a half-filled entry is someone leaving fields
out of a generated contract, and that must not pass as a deliberate
hand-authored one. Such an entry is still declared, because the manifest is the
inventory of every package a dialect models — see [The manifest is the complete
inventory](#the-manifest-is-the-complete-inventory). `node scripts/dialect-manifests.mjs validate` — part
of the universal check set — enforces all of that and fails on any declared
artifact that does not exist, so a half-added package cannot ship as a dialect
that silently models nothing.

`packagePathEnv` exists because this repository has no root `package.json` and
therefore no `node_modules` to read the audited releases from. Generation and
drift checks read a package path from that variable, falling back to
`defaultPackagePath`. Point each one at an installation of the exact pinned
version:

```sh
mkdir -p /tmp/contract-packages && cd /tmp/contract-packages
npm init -y >/dev/null
npm install --ignore-scripts --no-audit --no-fund \
  solid-js-1x@npm:solid-js@1.9.14 solid-js@2.0.0-rc.0 @solidjs/web@2.0.0-rc.0
```

```sh
SOLID_V1_SOLID_JS_PACKAGE=/tmp/contract-packages/node_modules/solid-js-1x \
SOLID_V2_SOLID_JS_PACKAGE=/tmp/contract-packages/node_modules/solid-js \
SOLID_V2_SOLIDJS_WEB_PACKAGE=/tmp/contract-packages/node_modules/@solidjs/web \
  make contracts-check
```

### Steps

1. **Declare it** in `rust/dialects/<id>/dialect.json`, as above.
2. **Teach the generator its semantics.** Add the `generatorTarget` and its
   reviewed callback/return tables to `solid-contract-gen`
   (`rust/crates/solid-facts-backend/src/bin/solid-contract-gen.rs`). Read the
   runtime implementation for each claim; a signature does not carry it.
3. **Generate** with `make contracts`, which writes the review contract and the
   Rust export index for every declared package.
4. **Register the export index** in `rust/crates/solid-dialect/src/exports/mod.rs`
   and consume it from the vocabulary implementation.
5. **Produce the bundled runtime contract** at the declared `bundledContract`
   path, and decode it in `diagnostics.rs`. Its evidence URI must be
   `bundled://<id>/<package-slug>.json`, matching the artifact path.
6. **Verify** with `make contracts-check` and `make contract-conformance`.

For a whole new dialect rather than one package, follow
[adding-a-dialect.md](adding-a-dialect.md), which wraps these steps in the
vocabulary, compiler, catalog, and detection work.

### Runtime probes and the lock

Set `probeRuntime` when the contract's claims are checked against an installed
release. `node scripts/check-bundled-contracts.mjs` then installs the exact
pinned release, checks its export surface and npm integrity, verifies every edge
in `pkg/contracts/bundled/runtime-lock.json`, and executes every declared
behavior probe in client, server, development, and production condition modes.

**Probing is grouped by dialect, one install root each.** A single shared
install cannot host them: `@solid-primitives/scheduled` peers on
`solid-js@^1.6.12` while the 2.0 contracts pin `2.0.0-rc.0`, and npm refuses the
combination. Each dialect installs its probed packages *and* its non-probed ones,
so a peer resolves to the release that dialect audits rather than whatever npm
would pick — and `runtime-lock.json` pins the transitive closure of both.

Each dialect names its probe worker in `scripts/check-bundled-contracts.mjs`.
The worker is copied into that dialect's install root and run from there, so its
bare `import "solid-js"` resolves to that dialect's release; the shared harness
in `scripts/lib/contract-probe-harness.mjs` travels with it. A worker cannot be
shared across dialects because driving a probe is version-specific: 2.0 settles
with `flush()`, and 1.x has no such function. A dialect that declares
`probeRuntime` contracts with no worker is an error, not a silent skip.

Probe identity is `(dialect, package, entrypoint, export, claim)`. The dialect
is part of it because both dialects declare `solid-js` at different versions,
and a name-only key would merge two different packages' observations.

**A contract may state its claims for fewer than the four modes**, via
`probeModes` on the manifest entry. Solid 1.x resolves a genuinely different
artifact under `node`: `createEffect` never runs, `createMemo`, `createComputed`,
`createRenderEffect` and `children` compute once and never re-run, and
`render`/`hydrate` throw a client-only error. The bundled 1.x contract therefore
states client semantics — `client`, `development`, `production` — and makes no
claim about the server build, which needs its own contract and does not yet have
one. Restricting the modes records that boundary; it does not weaken the ones
that remain, and any claim inside a stated mode is still probed.

**`inline`, `deferred` and `tracked` are one word over two axes**, and knowing
which axis decides which word is what makes a row checkable:

- **`tracked`** is an *attribution* claim: the export establishes its own
  tracking scope around the callback, so reads inside it subscribe that
  computation and it re-runs on its own. Nothing about when it first ran is
  promised — a tracked computation that has not run yet still owns the reads it
  will make.
- **`inline`** and **`deferred`** are *schedule* claims, and they only ever
  describe a callback the export does **not** subscribe. `inline` promises the
  export invoked the callback before returning; `deferred` promises it did not.

So `tracked` is decided first, on the attribution axis, and the schedule axis
then separates the rest. A probe measures accordingly: an `inline` probe puts
the export call inside a memo and checks the callback ran during the call; a
`tracked` probe checks the callback re-runs when a signal it read changes; a
`deferred` probe checks neither. `untrack`, `createRoot`, `runWithOwner` and
2.0's `flush` stay `inline` while *clearing* the listener — they run the
callback during the call, and the clearing travels separately through the
dialect's `runs_callback_synchronously`. Every claim of a probed contract is
probed; there are no exemptions.

The exception sits on the attribution axis, and it is where the dialect's own
`Execution` vocabulary and a contract row diverge. `Execution` (solid-dialect)
classifies attribution *only*, because its other consumer — the checker's
`callback_runs_outside_tracking` — asks nothing else. 1.x `startTransition` is
`Execution::Inline` and correct there: its callback runs in a
`Promise.resolve().then()` microtask, and the runtime restores the captured
listener, so a read inside subscribes exactly as at the call site.
`createResource`'s fetcher is `Execution::Deferred` even though the sourced
overload runs it during the call, for the same reason. Neither word states a
schedule, so **contract emission does not read one out of them**: both
primitives are absent from the schedule table in
`solid-reactive-ir/src/interproc.rs`, and a callback wrapped in either is
refused rather than published. Where emission does need a schedule it asks the
dialect for one — `runs_callback_synchronously` for the clearing-but-immediate
primitives and `tracked_callback_timing` for when a tracked computation runs
relative to the call that creates it — and publishes the unknown sentinel where
the dialect states none. That last fact is not cosmetic: in 1.x only
`createEffect` defers among the tracked primitives, so reading `tracked` as
"runs later" made `createMemo(() => untrack(cb))` claim `deferred` where the
runtime, and the probe, say `inline`.

The corollary is that a claim stated for one variant and not another is
registered only in that variant's modes. Solid 2.0's server build re-runs
nothing, so a browser body asserting `tracked` cannot pass there and a server
body asserting `inline` cannot pass on the browser; registering either
everywhere records a *failing* result for a claim no variant states in that
mode. The claim is the probe's key, so a wrong registration is not merely
noise — it is an observation filed against a question nobody asked.

**A bundled contract may carry an unknown claim, and the suite reads it as
one.** `{"status": "unknown"}` is a valid schema-v1 value for `callbacks`, and
it is the opposite of a row set: nothing to probe, and no negative for
discovery to contradict. `solid-js@2.0.0-rc.0`'s `merge` carries it, because
the export is variadic — it wraps *every* function argument in a memo — and
schema v1 indexes callback rows per parameter, so any finite row set certifies
a false negative at the first argument past it. A sentinel is the honest answer
where the runtime's shape has no encoding; it is not a way to skip writing a
probe for a claim that does have one.

`node scripts/check-contract-pins.mjs`, in the same target, covers what probing
cannot reach. The probe suite proves a package's identity by installing it and
reading npm's hidden lockfile, so a contract it does not install — a
hand-authored overlay, or a dialect whose runtime is not probed — would be
pinned by a version string alone. A version string is not a pin: republished or
mutated contents keep the version, and the contract would still claim to
describe them. So every bundled contract records the integrity of the exact
tarball it was audited against, that integrity is checked against the registry,
and a contract recording none fails.

`--write` records passing modes as `probed` row evidence on claims that already
exist. **It does not repair a lock or probe mismatch, and must not be taught
to.** A probe failure means the package does not behave the way the contract
says; a lock mismatch means the package that was probed is not the package that
was audited. Neither is drift in a derived artifact, and neither is fixed by
regenerating.

### Composed artifacts

When a bundled artifact is assembled from checked-in inputs rather than
generated directly from a package, declare `composeScript` and `composeInputs`.
`node scripts/dialect-manifests.mjs check-composed-contracts` runs each script
with `--check`, failing when the checked-in artifact is stale relative to its
inputs. The Solid 1.x contract works this way: it is composed from a per-subpath
export census and the reviewed semantics map.

### Version bumps

A pinned version is an audit boundary, not a dependency range. Moving one means
regenerating the artifacts *and* re-reading the claims the generator's tables
assert, because `--check` compares the artifact to those tables and has nothing
to say about whether they still describe the runtime. A newer prerelease is
reviewed, never silently substituted. Consumers see the same boundary from the
other side: an installed version other than the audited one is refused and
reported as a stale contract.

### The manifest is the complete inventory

Every gate above enumerates the contracts a manifest **declares**, which leaves
one hole they cannot see: a package a dialect models but no entry names is
covered by nothing at all. It silently has no contract, and every project
importing it reports `SC9005` forever.

`every_modeled_package_is_declared_in_the_assembly_manifest`, in
`rust/crates/solid-facts-backend/src/dialect.rs`, closes it by deriving the
expected set from the dialect instead of the manifest. A package is modeled when
the vocabulary owns one of its modules (`Dialect::modules`) or the backend
compiles a contract in for it (`Dialect::bundled_packages`); the two sets must
match the declared packages exactly. An undeclared modeled package fails, and so
does a declaration for a package the dialect neither owns nor bundles, which is
dead weight.

Module specifiers collapse to package roots first, the same way contract
discovery resolves them: `solid-js/store` and `@solidjs/web/frames` are subpaths
of one installed package, not packages of their own.

The check runs with `cargo test -p solid-facts-backend --lib`, and therefore in
`make verify`. It reads the checked-in `dialect.json` files directly, so it
fails on the manifest as committed rather than on a regenerated copy.

## Bundled and ecosystem contracts

Verified contracts for `solid-js` and `@solidjs/web` are embedded in the
checker and selected automatically from project imports. They pin Solid
`2.0.0-rc.0` and its npm integrity. The core contract covers the root and
refresh entrypoints; the web contract covers all 13 runtime entrypoints,
including server-functions, frames, serialization, storage, and the JSX
runtimes.

Run the exact-release conformance suite with:

```sh
make contract-conformance
```

It enumerates every non-pattern runtime entrypoint and conditional ESM build,
checks missing/stale exports and function/value kinds, verifies npm version and
integrity, and requires a passing behavioral probe for every callback and
reactive-return claim. Behavioral probes run the applicable client, server,
development, and production conditions independently, exercise both the first
and subsequent callback calls, and report their mode and call count. A claim
must pass in every mode selected by its entrypoint conditions. `--write` may
record those passing modes as `probed` row evidence; it never discovers or
adds an uncontracted behavior. The normal `scripts/verify.sh` workflow runs
its conformance half (`scripts/check-bundled-contracts.mjs`); CI's contracts
job runs the full suite on every push and pull request.

A bundled contract for Solid 1.x is embedded alongside them:
`pkg/contracts/bundled/solid-v1/solid-js.json` pins `solid-js` 1.9.14 and covers the
`.`, `./store`, and `./web` entrypoints. It is generated by
`scripts/generate-bundled-solid1-contract.mjs` from two checked-in inputs: the
runtime surface `pkg/contracts/bundled/solid-v1/solid-js-runtime-surface.json`,
which records which exports the installed release actually has under which
entrypoint, and the reviewed semantics map
`rust/crates/solid-dialect/contracts/solid-v1/solid-js.json`, which supplies the
callback and return summaries. The same
`make contract-conformance` target runs the generator with `--check`, which
fails when the checked-in artifact is stale relative to either input instead
of shipping a drifted contract.

Solid 1.x also embeds a narrow reviewed contract for
`@solid-primitives/scheduled@1.5.3`. It distinguishes deferred `debounce`,
`throttle`, and `scheduleIdle` callbacks from the inline scheduler factory
arguments used by `leading`, `leadingAndTrailing`, and `createScheduled`. The
contract is exact-version matched; other releases must ship or generate their
own contract rather than inheriting guessed timing.

The pinned Solid Primitives `next` corpus contains 98 packages. Its contracts
are generated from complete package export maps, including subpaths and
materialized conditional targets. Regenerate and validate the complete corpus
with:

```sh
make corpus
```

Set `SOLID_PRIMITIVES_CORPUS=/path/to/clean/checkout` to reuse a local clone.

Generation automatically discovers contracts from declared dependencies and
repeats to a fixed point so their transitive summaries are retained. Validation
checks every normalized contract and confirms each package manifest publishes
`solid-reactivity.json`; artifact hashes are also checked whenever a contract
contains them.
