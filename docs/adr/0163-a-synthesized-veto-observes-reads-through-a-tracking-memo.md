# ADR 0163: A synthesized veto observes reads through a tracking memo

- Status: accepted and implemented (2026-09-29); written with the implementation
- Date: 2026-09-29
- Owners: synthesized vetoes
  (`rust/crates/solid-facts-backend/src/contract_certification/synthesized_vetoes.rs`,
  `Observation::EmptyReads`, `empty_reads_module_source`,
  `tracking_runtime_in_workspace`, `every_slot_described`), the probe
  workspace's closure (`probe_harness.rs`, `authenticated_closure_snapshot`),
  and the dialect seam (`solid-dialect`, `Dialect::tracking_runtime`,
  `TrackingRuntime`)
- Relation: adds the fourth reviewed domain to ADR 0036 § 3's synthesized
  vetoes. It reverses the recommendation of
  `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`
  § 3-§ 4 on the owner's decision of 2026-09-29, and answers that document's
  two objections rather than setting them aside. ADR 0101's described `reads`
  enumeration is unchanged.

## Context

The implementation census decides a `reads: []` candidate, and ADR 0006 then
requires a mandatory veto: a finite runtime run that may contradict the closure
and never supports it. For every other empty domain the checker synthesizes
that veto from the export's call signature (ADR 0036 § 3). For `reads` it did
not, on the phase-21 design's reasoning, so every `reads: []` candidate whose
census passed waited for a hand recipe. On the `@solid-primitives`
checkpoint that was the largest wall: 296 exports under `node`, of which 134
had a census that passed and needed only a veto.

The owner decided on 2026-09-29: synthesize the veto; no hand recipes; the
census alone is not accepted.

The phase-21 design gave two reasons not to:

1. A veto derived from the call signature instruments only what the *caller*
   supplies, and `reads` is about sources the export **owns** (ADR 0034 puts a
   read through a caller's value on the caller's side). An observation of the
   caller's values runs clean for an export that reads a module-level signal
   on every call.
2. Seeing an owned read means knowing which sources a computation subscribed
   to, and those fields are minified and move between builds.

## Decision

1. **The observation.** Each sample call runs as the compute of a fresh memo
   of the workspace's tracking runtime, created under a fresh root, and the
   memo's own data fields are read once the compute has returned, before the
   root is disposed. The claim is contradicted when that memo gained a
   dependency: a tracked read of any source during the call, whether the
   module owns it, the call created it, or it is a memo the call created and
   read. The sample hands the export no reactive value of its own, so no
   dependency the memo gains can be a read through a caller's value (ADR
   0034). That answers objection 1: the observer is the one computation every
   tracked read in the call reports to, whoever owns the source.

2. **Calibrated, never named.** Which fields hold dependencies is found on
   every run. The module compares a memo that read one signal with one that
   read nothing and keeps the fields the first holds as objects where the
   second holds none. It throws, which withholds the candidate as an
   incomplete veto, when no field separates them, or when the same pair no
   longer separates them after the samples ran. The field names differ in
   every audited build (`nt`/`Ye` in rc.3's `dist/prod`, `ut`/`je` in rc.6's,
   `Se`/`ot` in rc.9's, `ee`/`Fe` in rc.9's `dist/observe`, `_deps`/`_depsTail`
   in each `dist/dev.js`). The same module calibrates on all seven, so no
   constant here has to follow a build. That answers objection 2.

3. **The runtime is the workspace's own audited copy.** The runtime is named
   by the dialect, not by shared code: `Dialect::tracking_runtime` is `None` by
   default and is `@solidjs/signals` with `createRoot`, `createMemo`,
   `createSignal` and `getObserver` for Solid 2. A `reads: []` veto is
   synthesized only when the gate batch's authenticated workspace carries that
   package as an audited archive (`audited_archive_for_snapshot`: name,
   version, integrity and manifest digest). The workspace carries one copy per
   name (`authenticated_dependency_closure`), so the package under test
   resolves the same copy. The synthesized entry declares the package in
   `dependencySpecifiers`, and the harness then requires the worker's
   resolution of it to land inside that copy. The graph lane passes the same
   dependency plans the gate pre-pass hands the batch, so synthesis and the
   workspace agree on what is present.

4. **Only arguments synthesized in full.** The veto is written only when
   every non-rest parameter of every signature has candidates its value
   facts describe (`every_slot_described`). A slot sampled with the
   unknown-input fallback (`undefined` and `{}`) would make a clean run a
   statement about values nobody said the export accepts. Rest parameters are
   sampled empty. Everything else keeps `no recipe in corpus`.

5. **A throw.** A throw inside the export is caught inside the compute, so
   what the call linked before throwing is still observed. A run in which
   every call threw and none read observed nothing. It throws and the
   candidate is withheld, never passed.

6. **Stated limitations.** The observation is exact for what it sees and not
   exhaustive, so its reviewed text does not begin `exact:`. It does not see:
   an untracked read (`untrack` links nothing); a read the export defers (a
   microtask, a timer, an effect's later run, an `await`), because the root is
   disposed before anything is flushed; a read performed by a computation the
   export creates and never reads back, which is that computation's; a read of
   a non-reactive value the export owns (a plain `Proxy` or getter, which the
   census refuses on its own, `implementation-census-reads`' `./owned`); and a
   read through a second copy of the runtime that the package bundles. The
   entry carries all of these, and the fixture pins the first three as
   behaviour.

## Consequences

- `fixtures/package-contracts/synthesized-reads-veto` is the proof that the
  veto can fail, run against the real rc.9 bytes. `readsOwnSignal`,
  `readsCreatedSignal`, `readsCreatedMemo` and `readsThenThrows` are
  contradicted and `readsNothing` is clean. `throwsWithoutReading` is
  incomplete. `readsUntracked`, `readsLater` and `createsAReadingMemo` are
  clean, as the limitations say. The same module calibrates on rc.3 and
  rc.6 too. A stub runtime whose observer never changes makes calibration
  throw.
- `implementation-census-reads`' `plainArithmetic` certifies with no hand
  recipe once the workspace carries rc.9's audited `@solidjs/signals`. It stays
  withheld for want of a recipe when the workspace carries no tracking
  runtime, or carries one at an integrity that is not audited.
- The probe-harness image is unchanged: the module is a corpus entry, and
  declared dependency specifiers already existed. No pin moved.
- A contradiction from this veto refuses the row, as any veto contradiction
  does. The census is the proof, so a contradiction here means the census and
  the runtime disagree. That is a defect worth a refusal.

## Measured (2026-09-29, `make primitives-checkpoint`, release binary)

This was measured against the ADR 0156 base with the seroval workspace fix
(`a2a2335d`) applied. Clean exports out of 721:

| host | before | after |
| --- | ---: | ---: |
| none | 96 | 91 |
| browser | 96 | 91 |
| node | 96 | 101 |

The veto contradicted a closure the census had proved in the wild on its
first run. `@solid-primitives/date@3.0.0-next.3`'s `createCountdown(a, b)`
does `difference = createTimeDifference(a, b)[0]` and then reads
`difference()`, a memo the call created, at the call. The census passes its
`reads: []`. The synthesized sample `[callback, callback]` takes that branch,
the observing memo gains the dependency, and the row refuses under `none` and
`browser`. The row had 9 clean exports; that is the whole of the drop on those
two hosts. The gains are 4 exports under `none`/`browser` and 5 under `node`,
plus 18 to 27 packages whose `reads` closes on at least one more export
without the export turning clean.

The finding is the census's, not the veto's: `census_reads_domain`
dispositions no call ("this census dispositions no call and recurses into no
declaration"). `semantic-model.md` § reads scopes the domain to reads
"arising from non-call syntax", so a read made by calling an accessor, or by
calling a local or dependency function that reads, is not a form the census
enumerates. The owner's statement of this veto counts any dependency the call
gains. The two definitions disagree on every call-made read, and this
veto is what exposed it. Which definition holds is an owner decision (see
the backlog). Until then, every `reads: []` this veto lets through rests on a
census that did not look at calls, and on a finite sample that did.

## Alternatives rejected

- **The dev build's `DEV.getSources`.** It names the fields, but only under
  the `development` condition. That condition moves every conditional target
  in the closure off the bytes the Type Facts witness read, and the harness
  admits only reproduction conditions that move none (ADR 0037).
- **Hand recipes.** The owner declined them: they do not scale to 134
  exports, and each one is a runtime claim authored by hand.
- **Watching caller-supplied tripwires** (the phase-21 design § 2). This is
  objection 1: it runs clean for every owned read.
