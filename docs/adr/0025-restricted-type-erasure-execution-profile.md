# 0025 — Restricted type erasure requires a consumer execution binding

Status: investigated; production admission blocked, refusal retained
Date: 2026-09-04

Subsequent decision: ADR 0026 admits the narrower `node-strip-inert-esm-v1`
profile only through a checker-owned controlled invocation. The general
import-free profile reserved here remains unadmitted. This ADR records the
pre-implementation blocker and its broader requirements.

## Decision before implementation

Reserve the design name `node-strip-esm-import-free-v1` for the restricted
interpretation below. It is **not an accepted execution profile** in this
release. Do not install a loader, issue derived-execution receipts, or let a
profile name or matching digest make a source-case gate pass. Pin the concrete
counterexample and the existing consumer's rejection of unknown profile fields
in regression tests. ADR 0009's source refusal and ADR 0021's sandbox scheme 7
remain in force. This is the latest task's expressly permitted outcome when
consumer compatibility cannot be established; it is not a finding that erasure
cannot work.

The defensible scope is execution by one exact interpreter of one exact derived
module under one exact resolution and runtime model. The checker currently
authenticates a consumer's source-artifact resolution, not that execution. A
matching source, transformer hash, compiler name, tsconfig, or user opt-in does
not establish that the consumer executes the derived module. The reflection
control supplies a concrete counterexample. Issuing the existing kind of
receipt would therefore grant a stronger claim than the evidence supports.

## Evidence and alternatives

The completed POC at
`/private/tmp/solid-checker-type-erasure-poc-01a06ca3/` issued no receipts and
changed no repository files. Its results JSON hashes to
`dff784a6686446836d32da283b181ac3957d308e004030bc8c76769a6613cf35`.
All 40 original TS candidates parse after Node 24.11.1 strip-only processing;
32 are import-free and loaded, 26 completed finite samples twice, six needed
browser APIs, and eight were withheld for extensionless relative imports.
There were zero real-package contradictions; all 17 controls passed twice.
Neither a native implementation census nor native receipt validation ran.

The strongest negative control uses `Function.prototype.toString`: stripping
leaves annotation-width whitespace while TypeScript 5.9.3 emission does not.
The stripped execution has only call enter/exit; the compiled execution adds
`create-operation`. This is a successful falsification of cross-profile reuse,
not a failed eraser and not a real Kobalte contradiction. The POC's global-key
tripwire does not observe arbitrary registrations or all initialization effects.

| option | what remains justified | cost / lost claim |
| --- | --- | --- |
| Keep the refusal | unchanged published-case census and mandatory veto | 40 source candidates remain unavailable to the existing execution tier |
| Census alone, veto withheld | a different static-only policy, if separately reviewed | loses independent contradiction detection; current withheld closures are open, not certified negative claims |
| Certify published JS | census and veto concern that exact JS artifact case | 40/40 source candidates have same-named JS siblings, but 0/40 have proved equivalence; prior work independently closed five 0.9.2 browser-JS domains and two alpha clamp domains |
| Restricted erasure with only a profile digest / opt-in | identifies an experimental derived execution | cannot establish compatibility with the consumer; rejected |
| Restricted erasure with a verified consumer execution binding | scoped census plus veto over the exact execution described below | viable design, but the consumer boundary has no such evidence or enforcement today |
| Relocate TS or infer import extensions | can make a particular experiment load | changes interpretation/resolution and still requires the same consumer binding; no shortcut |

## The proposed scoped certificate

If admitted in a future change, a certificate would assert that the listed
claim domain is closed for **derived ESM bytes**, under the named execution
profile, guards, argument/resource scope and exact dependency environment.
It would retain the native universal implementation census and a mandatory
contradiction veto. Finite non-observation would never discharge a demand.
It would make no claim about execution by TypeScript, esbuild, Babel, Bun,
another Node version, a browser, or a subsequent minifier/bundler.

Source-only census evidence cannot silently become derived-byte evidence.
Either acquire and authenticate a census over the actual derived runtime with
an explicit source/output binding, or establish a reviewed erasure-preservation
premise for the census's invoking-form model. This includes module
initialization and erased imports. Unsupported syntax and unsupported census
forms still refuse. Merely checking that both inputs parse proves neither.

The profile must bind all of the following, with canonical domain-separated
identities derived by the native verifier, not supplied as authoritative strings:

- Authenticated archive integrity, manifest, artifact case, source path and
  original bytes; each derived module's exact bytes and source correspondence.
- The compiled-in Node executable pin, reported version, platform/architecture,
  pinned loader/transformer implementation and its full executable closure.
  Use Node `stripTypeScriptTypes` in `mode: strip`; no lowering, source maps,
  sourceURL addition, tsconfig interpretation, or automatic fallback compiler.
- ESM format and exact source URL/module identity. Keeping a source URL is a
  deliberate interpretation override, not proof of unchanged published bytes.
  Refuse TSX, enums and other syntax requiring transformation. Refuse CommonJS.
- An import-free runtime module graph established after erasure by a complete
  parser census, including dynamic imports and executable require forms.
  Type-only import elimination is part of the profile, never an assertion of
  equivalence to compilers that preserve different initialization behavior.
- Exact package resolution, observed Node conditions, export bindings and the
  authenticated dependency closure. A recipe's resolution echo must still
  equal the original selected source target; output identity is an additional
  check, not a replacement for `verify_reported_resolution`.
- Runtime environment and sandbox policy, recipe bytes, all gate IDs and
  outcomes. No DOM shims. A browser-dependent function needs another execution
  profile, not globals chosen to satisfy its sample.

Before publication, any transformer/input/output/profile mismatch must refuse
inside the native transaction. Mismatching pins must never trigger installation,
repinning, or a fallback execution. Source and output copies plus the loader
must join the watched census. Preserve the 0700 workspace, compiled pins,
env_clear/allowlist, process group/killpg, single startup and single run frame,
primordial capture and frozen prototypes before recipe import, and
detect-and-refuse isolation on every exit. POC Node permissions are not a
substitute for those properties or an OS sandbox claim.

## Consumer applicability is the blocking premise

`contract_interface::load_authenticated_policy2_contract` authenticates the
receipt, recomputes `policy2_resolved_import_root`, and replays artifact,
closure and export selection. `ResolvedImport` carries source runtime and
declaration files, optional transform file identity and resolution traces.
It has no observed source-to-final-output graph or launch enforcement token.
`AcceptedContractIndex` keys accepted semantics by import and receipt identity;
there is no execution-profile compatibility check at query time.

`finalization::finalize_value_only` currently writes
`empty("empty-transform-schedule")` into `transform_root`. The receipt loader
compares that root with expected signed/catalog bindings; it does not turn a
nonempty root into a verified build transcript. Repurposing this hash would
permit self-consistent metadata without establishing applicability.

A sound first consumer could be a checker-owned launcher that enforces the
same closed module inventory, exact Node/loader pins, source/output bytes,
resolution rules and runtime assumptions, and carries a non-forgeable
transaction capability into contract loading. Its contract is conditional on
that controlled execution. Another possible consumer is an independently
verified build/execution transcript proving the final bytes and module
identities, with no unverified downstream transforms. Neither exists here.
Adding only `executionProfile` to a config file establishes a request, not
either premise. Arbitrary consumer builds remain unsupported even if they
declare the same Node version or strip-only intent.

This is a concrete missing mechanism, not a demand to prove equivalence of
all TypeScript compilers. The next implementation must own that consumer
boundary as well as the certifier. Until then there is no supported consumer
for which this repository can establish the new premise.

## Compatibility and identity migration

Current receipt payloads, accepted-catalog bindings, resolved imports and
recipe entries use `deny_unknown_fields`. An explicit `executionProfile`
extension must fail decoding, including when it says `published` or `null`;
it cannot be ignored or treated as an opt-in. Regression tests exercise the
active receipt consumer and the native binding decoders. Existing consumers
also refuse unknown receipt versions and proof-policy digests.

An admitted implementation needs a new receipt/proof-policy identity and an
explicit profile in normalized applicability and consumer cache keys. Old
consumers must refuse before granting any knowledge. A profile buried only in
`probe_gate_root`, `transform_root` or coverage limitations is insufficient.
The worker protocol must bind source/output/profile resolution data and reject
missing fields. The sandbox scheme must advance from 7 with an amended ADR
0006 disposition table for the loader, format override and derived watched
inputs. The public contract schema migration must follow AGENTS.md atomically,
not reinterpret stable version 1 or rely on ignored optional fields.

This change admits no profile and changes no wire meaning, policy field or
protocol. Therefore none of those identities is bumped here. It adds refusal
and reflection regressions, not a schema extension that existing consumers
could accidentally accept.

## What transformation can hide

Even strip-only processing changes reflected function source and locations;
type-only import removal changes the set of modules initialized. More general
transforms can change field/decorator initialization, enum reads, coercion,
iteration and helper calls. A transformer defect can remove the very operation
the veto should observe. Restricting syntax reduces these hazards but does not
prove cross-profile preservation. A scoped certificate remains defensible only
because its consumer executes those exact derived semantics and its census
proves that same claim; it never transfers a clean observation back to another
compiler's output. The reflection fixture must stay a cross-profile refusal.

## Measurements and remaining work

The importing population was investigated separately without executing it.
Re-hash the retained 0.9.2 `package.tgz` against the original SHA-512 integrity,
read its members directly, strip and parse the three importing source modules
and their transitive targets. All six modules parse; there are exactly three
static edges, each target has no further static runtime imports:

| candidates | source | written specifier | unique published sibling |
| --- | --- | --- | --- |
| isVirtualPointerEvent (1) | src/is-virtual-event.ts | ./platform | src/platform.ts |
| scrollIntoView, scrollIntoViewport (2) | src/scroll-into-view.ts | ./get-scroll-parent | src/get-scroll-parent.ts |
| getAllTabbableIn, hasFocusWithin, isElementVisible, isFocusable, isTabbable (5) | src/tabbable.ts | ./dom | src/dom.ts |

All three exact extensionless targets are absent. The measurement file
`/private/tmp/erasure-import-investigation.json` retains each source/output
digest and the archive integrity. It is a static-edge inventory, not proof
against dynamic loading and not a resolution capability. Native
`module_closure::resolve_local` already enumerates suffix candidates for
analysis; Node's ESM loader does not inherit that algorithm. A future importing
profile must authenticate an edge map keyed by importer identity, specifier,
conditions and module format, compare actual loader results against that map,
reject ambiguous/missing edges and escapes, and enforce the same mapping for
the consumer. Hashing the archive alone cannot authorize suffix inference.
No importing candidate is newly admitted; browser APIs are still a separate
question after successful resolution.

The new before/after runs use the same checked-in recipe corpus and the three
original candidate-bearing rows. The ecosystem baseline without a recipe corpus
is not a baseline for this experiment. The accompanying dated report records
the fresh refusals, counts, import-edge investigation and checks. POC samples
remain feasibility evidence, never certifications. Existing published-JS
closures and the independent accessor-census refusals are preserved.
