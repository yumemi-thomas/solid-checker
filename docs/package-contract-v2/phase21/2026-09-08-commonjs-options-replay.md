# Partial JSON initialization replay prototype

This is a diagnostic implementation of the first transfer rules in ADR 0088,
not a certification lane. No production verifier, protocol, receipt or
accepted catalog changed. The latest full measurement remains 327 complete
rows and 1,535 accepted artifact cases.

The prototype statically replays the retained Partial JSON 0.1.7 `options.js`
from the latest full report's TanStack AI Solid project. Its SHA-256 is
`779b2aded6d4e2ec6171ea9d5d5d97551e8c3258b6e78a01d2e9fc6235865b36`.
It uses Acorn to parse source without evaluating package code. Ordered steps
cover nested assignments, `void`, numeric bitwise OR, own member reads,
object-literal data definitions and the restricted `defineProperty` descriptor
used by this file. Reference evaluation precedes right-hand-side evaluation.

Under an explicitly hypothetical complete empty prototype and exact intrinsic
premise, it replays 32 writes/definitions and obtains all 17 own export names,
including `__esModule`, fourteen numeric constants, `Allow` and `default`.
The serialized resulting values match a separate Node 24.11.1 runtime control;
`ALL` equals 511. That value comparison does not establish reference identity,
runtime semantics for other inputs or a consumer's host premises.

Six negative controls refuse an unknown prototype, an inherited setter,
an inherited descriptor field, a local `exports` declaration, `eval`, and the
unsupported variable-declaration initialization in the real `index.js`.
An absent entry in a partial host observation is never interpreted as a
complete prototype census. Source, statement, expression-count and expression-
depth caps bound the prototype. It has no authority to validate host records.

The source and successful result are retained at
`/private/tmp/commonjs-binding-controls/options-replay.mjs` and
`options-replay-result.json`. The pinned Node command exited 0. This prototype
is not wired into generation, so it cannot clear a refusal or affect coverage.
It provides a concrete transfer-rule implementation to port into the semantic
owner once exact host/source evidence is available, rather than an export-name
heuristic. Next required rules include local declarations, function/class
values, require dependencies and the actual getter-reexport helper behavior.

Independently, a temporary Go overlay requested implementation transcripts
directly at the exact published `parseJSON` and `_parseJSON` function subjects.
It passed in 0.034 seconds. Both subjects have complete structural transcripts,
with 5 and 70 calls respectively. This is producer census completeness, not
contract acceptance: the inner parser still records 65 unknown-accessor and
30 coercion forms under its unrefined JavaScript parameters. Both direct
runtime queries explicitly refuse the declaration-file parameter premise.
A future certification must join the real declaration signature and proved
helper argument values; it cannot replace these open facts with the public
parameter type by name. The overlay and log are
`implementation-overlay.json`, `implementation_transcript_test.go` and
`implementation-observation.log` in the same temporary directory.

## Two-module replay

The subsequent reviewable prototype is retained as
`prototypes/commonjs-module-replay.mjs`. Run it from the repository root with
`node docs/package-contract-v2/phase21/prototypes/commonjs-module-replay.mjs rust/target/ecosystem-investigations/2026-09-08-original-helper-full.json`.
It follows the supplied report's exact parent project pointer, checks parent
and dependency names/versions and records the report and runtime file hashes.
No installed package files are altered. The require-based resolution remains
a diagnostic anchor, not evidence for the certification case's import branch.

It now replays both real modules under the simulated host: 57 assignments and
definitions produce all 20 own export names, matching the runtime control.
Non-callable values match; callable comparison checks runtime kind only.
The replay separately identifies `parse` and `parseJSON` as the same symbolic
function from `index.js`, byte range 1568–1901. The error classes originate
at 1057–1092 and 1128–1165. These are initialization identities, not proofs of
parser behavior, constructor effects or later mutations.

The added transfer rules cover local bindings, function values, empty error
classes, one exact local require, restricted descriptor conversion, getter
calls and the actual `__createBinding`/`__exportStar` control flow. Every
write/definition retains its module and UTF-8 byte span. Source size, AST size,
statement count, execution fuel, recursion and enumeration have explicit caps.
The prototype rejects unsupported operations rather than executing source with
JavaScript `eval` or a VM. Runtime package loading is used only by the separate
comparison control after replay.

Six new refusal controls cover temporal dead zones, constant reassignment,
unsupported block lexical scopes, unknown requires, cyclic initialization and
eval. A distinct-function control confirms two separate functions are not
reported as aliases. The retained prototype command exits 0, and
`git diff --check` passes. The resulting observation is retained at
`/private/tmp/commonjs-binding-controls/retained-module-replay-result.json`.

This remains a prototype, not a general JavaScript interpreter or a producer
of trusted host evidence. In particular its simulated prototype/intrinsic
state, module cache and getter execution cannot be used as premises for an
ordinary consumer. No acceptance branch consumes these results. Production
integration still requires the agreed source/host evidence model, independent
replay and receipt binding, plus the parser's callable obligations. Full
verification was not repeated for this documentation-contained diagnostic;
the last production verification and full-corpus results remain ADR 0087.

## Authority and transfer-rule review

The existing `HostResolutionAdapter` is a structural resolution-row adapter;
its `Host` enum does not attest an initialized realm or module cache. The
current receipt path therefore cannot consume this prototype's simulated
host as trusted evidence. Shared host/proof/receipt ownership was requested
under the user's coordination requirement before changing those interfaces.
That request does not authorize weakening ordinary snapshot export replay.

Review corrected two prototype transfer errors before any production use:
an assignment to an existing writable own property preserves its descriptor
attributes, and a data-to-getter conversion retains omitted existing
enumerability/configurability attributes. Global bindings now live in a
shared environment across required modules. A module-local `const Object`
shadows that environment; assigning the global `Object` changes what a later
required module sees. The latter control refuses the resulting unknown call.
The complete two-module surface comparison still passes, alongside seven
refusal controls, three property/global controls and the distinct-function
control.

Four independent fresh Node 24.11.1 controls confirm the descriptor and
global-versus-local behavior and exit 0. An initial `node -e` control placed
the lexical binding in eval's global environment, so it was not the intended
CommonJS-local scope and failed. The corrected controls use an explicit
function wrapper. This reinforces that wrapper interpretation is a required
host premise; source text alone does not choose its binding environment.
No accepted cases or coverage counts changed during this review.

## Source-bound host requirements

The user confirmed shared-interface ownership on 2026-09-09. That blocker is
resolved. The next implementation step instruments replay to enumerate its
host premises instead of concealing them in the simulated realm.

The two published modules produce 354 source-bound requirement occurrences:
169 property-descriptor checks, 107 prototype-link checks, 63 global-binding
checks and 15 uses of `Function.prototype.call`. Each occurrence carries its
module and byte span. Descriptor checks distinguish a required present slot
from a required absent slot and state attributes, intrinsic identities and
host-object identities where those determine the replay. This list is
regenerated from source; callers cannot choose which requirements to omit.

An observational validator captures the relevant Node state before package
loading and compares it with the generated requirements. It passes for the
retained target and rejects three mutations: dropping an observation, adding
a property required to be absent, and substituting the intrinsic function.
The first comparison exposed that the prototype incorrectly modeled
`Object.prototype`'s property on the `Object` constructor as writable and
configurable. Both attributes are now false, matching the captured descriptor.
The corrected command and `git diff --check` exit 0; the 20-export comparison
remains unchanged.

This comparison is not an authenticated host session. The captured intrinsic
references are observational anchors, not independently proven primordial
identities, and the validator does not attest importer, loader, realm lifetime
or module-cache state. It therefore still produces no receipt authority.
Production integration must bind those identities and independently regenerate
the source requirements; hashing or signing an arbitrary caller-supplied list
would not establish their completeness or truth.
