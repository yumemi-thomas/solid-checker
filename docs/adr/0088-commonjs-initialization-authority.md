# CommonJS initialization authority

Status: proposed; no acceptance rule or public interface changes yet.

Ownership: on 2026-09-09 the user confirmed this task owns the CommonJS
host-evidence, proof-replay and receipt/consumer interfaces. The coordination
block is resolved; further confirmation is not required for this work.

The next complete-row candidates are Testing Library through Aria Query and
TanStack AI Solid through Partial JSON. Their runtime exports are CommonJS.
The current snapshot export replay and generator refuse these surfaces.
Removing that refusal would not establish the initialized values used by a
consumer. The expected ceiling is two rows, not a measured gain.

## Decision

Separate CommonJS value initialization from resolution and export-name
discovery. A file hash, compiler symbol, detected namespace key, or successful
probe cannot supply initialization authority. Preserve the existing artifact
case and dependency trust checks. A CommonJS binding may enter semantic
certification only after replay establishes the exact initialized value for
the consumer's access mode.

`ResolvedImport` currently binds importer, package artifact, resolution traces,
closure, transform and export selections. It has no initialization-state
record. `SnapshotVerifiedExports` independently replays selections from bytes;
a host-supplied export map alone must not bypass that replay. The probe
harness freezes intrinsic prototypes for its own reporting threat model;
this does not establish the ordinary consumer's initialization environment.

The necessary evidence has three owners:

1. The runtime host establishes module interpretation, wrapper bindings,
   initialization identity and cache selection. Its record distinguishes
   an ESM named snapshot, ESM default/module object and CommonJS require result.
   Compiler emit format cannot substitute for loader interpretation.
2. Snapshot replay proves value origins through the ordered initialization
   of the exact closure: assignments, property definitions, local requires,
   interop helpers and reexports. It must bind applicable property semantics
   and builtin operations; spelling `exports` or `Object` is not identity.
3. Receipt production and ordinary consumption bind the host premise and
   replay evidence to the exact importer/case, dependencies and trust. A
   consumer without a matching premise refuses. A fresh probe launch cannot
   authorize a different application's preexisting module state.

These are required semantic distinctions, not a proposed public JSON format.
Ownership is agreed as recorded above. No schema version change follows from
this design document.

## Bounded implementation path

Start with a host-backed, acyclic static CommonJS closure and exact declared
exports. Implement an ordered initialization replay that returns either a
binding with its positive premises or a source-bound refusal. The first
supported values must include the shapes needed by the actual targets:
function/class assignments, primitive constants, plain records, exact local
require values, and positively modeled getter reexports. A function-only
prototype is useful for tests but does not complete either target.

Prioritize Partial JSON before Aria Query. A fresh Node 24.11.1 diagnostic
followed the latest report's retained project pointers, checked each parent
manifest, resolved the dependency from that installation and traversed the
loaded CommonJS module children with a 512-node cap. Partial JSON loaded two
modules totaling 11,225 bytes; Aria Query loaded 150 totaling 158,252 bytes.
The exact source hashes and edges are retained in
`/private/tmp/commonjs-binding-controls/retained-local-closure-result.json`,
produced by `retained-local-closure.cjs` in the same directory. The command
exited 0. These are observed require-load graphs, not exhaustive static
closures, authenticated import-condition selections, or expected-case censuses.
The first attempted diagnostic used require resolution on the ESM-only parent
and was rejected; the final version explicitly uses the verified parent
manifest as a diagnostic resolution anchor and makes no parent-runtime claim.

Partial JSON's two-file observation makes its local constants and getter
reexports the first implementation target. Certification must still replay
all actual required initialization and callable obligations; the smaller
observed graph does not justify skipping any of them. Aria Query remains
within scope after that implementation is measured on TanStack AI Solid.

Unknown initialization calls, cycles exposing partially initialized objects,
escaped export objects, ambiguous wrapper bindings, unsupported getters and
unproved inherited property behavior retain explicit refusals. Supporting
one of them requires its own positive rule, not deleting the refusal branch.
Existing ESM replay and accepted artifact selections must remain independent.

Before connecting the replay to generation, prove mutation controls for the
host premise, access mode, initialization order, final export object, source
hash, dependency selection and cache identity. Then run each exact published
dependency, inspect the next refusal, and only then rerun its parent probe.
Count a complete row only after every required entrypoint is selected and
authenticated by the ordinary consumer. No estimate substitutes for that run.

## Evidence affecting the design

The controls in
`../package-contract-v2/phase21/2026-09-08-commonjs-binding-frontier.md`
already distinguish compiler exports from Node namespace keys, initial
`void 0` declarations from final assignments, and inherited setters from own
property creation. Neither target is reduced to a direct assignment pattern.

A further control on the exact retained Partial JSON 0.1.7 entry loaded it
with Node 24.11.1 `require`, replaced the loaded cache entry's `exports`, and
then imported the same absolute file through ESM. The require result used the
replacement; the ESM named `parse` and default object retained the original
values. Resolution and runtime bytes were unchanged (SHA-256
`23f750aa1170c830b7ef180135852317886cc58ce7d2597e5eeee044539072c7`).
Thus even a single current `require.cache` export object is not authority for
all access modes. This is a version-specific observed distinction, not a
claim about every Node version or a new package defect.

The initial experiment expected the ESM import to observe the replacement
and failed; the corrected assertions explicitly verify the observed split.
The final command exited 0. Source and result are retained as
`/private/tmp/commonjs-binding-controls/published-cache-selection.mjs` and
`published-cache-selection-result.json`. No package files were changed and
the in-process cache replacement was restored in `finally`. These runtime
observations guide proof design and confer no receipt authority.

No new certification or metric change is produced by this design. The latest
measured baseline remains ADR 0087: 327 complete, 64 partial, 18 refused and
9 not advanced rows, with 1,535 accepted artifact cases.
