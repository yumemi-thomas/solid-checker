# ADR 0099: A value export that cannot be invoked

- Status: accepted and implemented (2026-09-13); written with the
  implementation
- Date: 2026-09-13
- Owners: Type Facts producer (`not_callable_values.go`,
  `export_value_transcripts.go`), certifier (`type_facts.rs`
  `census_not_callable_export`), veto synthesis (`synthesized_vetoes.rs`
  `Observation::NotCallable`)
- Relation: the vacuous case of the rule every proposable call domain
  restates in `semantic-model.md` -- "one invocation of this export gives rise
  to …" -- applied to an export that has no invocation. Handshake protocol
  55 → 56.

## Context

The ecosystem ledger of 2026-09-13 carried 2,977 `callbacks` candidates and
several hundred `reads` candidates withheld as `no recipe in corpus`, and on
every hub certified standalone they were the same exports: `isDev`,
`isServer`, `EQUALS_FALSE_OPTIONS`, `INTERNAL_OPTIONS`, `alignments`,
`placements`, `sides` -- **values**, not functions. The generator proposes
their empty `callbacks` and `reads` closures exactly as it does for a function
export, and nothing downstream could ever close them:

- `synthesize` (ADR 0036) builds a veto only for an export that stated a call
  signature (`evidence.call_signatures`); a value states none, so the
  candidate was weakened out as recipe-less before planning.
- Had a recipe existed, the census would have refused the export anyway: the
  producer answered the implementation demand with `callSignatureNotUnique`
  because `GetSignaturesOfType` found nothing to walk behind, and
  `require_named_export_implementation` reads that as
  `domain-exhaustiveness … runtime implementation transcript is incomplete or
  open`.

Both are the same missing fact stated twice: the export cannot be invoked.

## Decision

**An export whose value type has no call and no construct signature closes
its empty proposable call domains on that fact alone, with a runtime `typeof`
veto as the second witness.**

Every proposable call domain -- `creates`, `returns`, `reads`, `callbacks` --
denies that *one invocation of this export* gives rise to an operation of its
kind. A value with neither [[Call]] nor [[Construct]] has no invocation, so
the denial holds for each of them without a body to census. This is not a new
claim; it is the existing claim evaluated over an empty set of invocations, and
the ADR's whole content is *when the producer may say the set is empty*.

### What the producer states

`ExportImplementationTranscript.notCallableValue` (`{ kind, type }`), beside
`declaration` for identity and with `valueNotCallable` as the transcript's only
open reason, when **all** of the following hold for the type of the runtime
binding on the runtime program:

1. `callabilityOfType` answers `NonCallable` -- the classifier the parameter
   facts already use, which refuses `any`, `unknown`, `never` and error types
   and answers `UntypedCallable` for the `Function` interface;
2. no constituent has a construct signature, and the export's symbol is not a
   class -- the type at a class declaration's own name is the *instance* type,
   which has no construct signature while the exported value is the
   constructor (the fixture's `Box` found this: the runtime veto contradicted
   the fact, which is the design working);
3. no constituent is instantiable (a type parameter or a deferred indexed,
   conditional or substitution type has no signatures *yet*, which is not the
   fact that it has none).

`kind` is `primitive` when every constituent is a primitive type and `object`
otherwise; `type` is the checker's spelling, for the receipt. Any doubt falls
through to `callSignatureNotUnique` as before.

### What the certifier does

`census_not_callable_export` runs before the per-domain census arms of the
`DomainExhaustiveness` family and answers `Some(sites)` only when the fact is
stated in exactly that shape, the proposal closes the demanded domain with an
**empty** enumeration (a value export with a described operation is a
proposal this premise does not reach), and the stated declaration is the
snapshot-replayed runtime binding of the demanded export by the same suffix
test the implementation accessor applies. The site is
`typefacts-value-export:not-callable:<kind>:<type>`; nothing is walked.

### What the veto observes

`Observation::NotCallable` synthesizes a module that imports the export and
emits `callable-value` when `typeof` of the runtime value is `"function"`. It
samples nothing -- there is no call to make -- and it is the one way the stated
fact can be false at run time: a value that is a function after all. The
mandatory-veto policy is kept whole; a not-callable export is not exempted
from it, it gets the veto that fits.

## What refuses, and why each is a refusal rather than a skip

- **A class.** Excluded by symbol flag before the type is asked, and by the
  construct-signature test after it. Pinned by `Box`.
- **A callable alias with an overloaded signature** (`entries = Object.entries`).
  `callabilityOfType` answers `Callable`; the transcript stays
  `callSignatureNotUnique` and the candidate stays withheld. Pinned by
  `entries`.
- **An `any`-typed value** (`parsed = JSON.parse("1")`). The classifier
  refuses `any`; no candidate is even proposed for it, since its shape is
  `unknown`.
- **A value export with a non-empty proposal.** The premise closes empty
  enumerations only.
- **A fact beside other open reasons, or on a complete transcript, or without
  a declaration.** Read as "not stated": a producer that disagrees with itself
  does not get the weaker reading.

## Consequences

- Protocol 56; `typefacts-v1.schema.json` gains `notCallableValue` and the
  frozen schema hash moves in both handshakes.
- The tracer is `fixtures/package-contracts/value-exports`; its two
  certification tests in `contract_certification.rs` pin the closures, the
  sites, and the three refusals, and `synthesized_vetoes_tests.rs` pins that
  the module is quiet on an object and loud on a function and on a class.
- **Cost, and one optimization refused.** Every value export now schedules a
  gate per proposable domain -- two per export, `callbacks` and `reads` -- and
  the harness runs each gate `repeatRuns` times in a fresh worker, so the
  corpus grew by roughly three thousand launches. The first release-binary run
  went from 854 s to 1,198 s wall, and the heaviest row came within three
  seconds of the 1,200 s row timeout, which the Makefile therefore raises to
  1,800 s. Running identical launches once -- the two gates of one export share
  module bytes, resolution subject, mode and repeat index -- was implemented
  and withdrawn: the evaluator's isolation invariant reads a duplicated run as
  a reused worker process (`repeat runs reused process, realm, or
  module-instance state`) and refuses the veto. Teaching it to recognise a
  replay would loosen an isolation guard, so the cost stands until the schedule
  can carry one gate for several claim ids.
- **A refusal this exposed.** `@solid-primitives/date-difference@1.0.2` had
  never run a probe gate; its first synthesized one hit an export-condition
  mismatch (the worker resolves `dist/index.cjs` where the witness read
  `dist/index.js`) and the plain lane refused the whole row where the graph
  lane would have withheld the served candidates. The plain lane now does what
  the graph lane does: `synthesized_cannot_run_withholding` withholds every
  candidate the hand corpus left recipe-less with the harness's own reason,
  drops the synthesized corpus, and the row certifies with those domains open.
  The batch path also routes a not-callable export to the per-plan loop, as it
  already did for one with a call signature; without that, a multi-case row
  never synthesized its veto.
- The corpus effect is measured in `docs/precision-backlog.md` (2026-09-13,
  ADR 0099 entry) after the release-binary run.
