# Compiler facts

Compiler facts describe execution behavior created or selected by Solid's
compiler. They are not syntax guesses and they do not describe arbitrary
package-runtime behavior. Solid 2 facts come from the semantic-only
[`yumemi-thomas/solid`](https://github.com/yumemi-thomas/solid) fork of the
official compiler under `packages/compiler`; Solid 1 facts remain on the
separate
[`solid-1x-compiler`](https://github.com/yumemi-thomas/solid-1x-compiler) fork.
Both are exact Cargo revisions and run in-process without Node-API types.

The public checker boundary is compiler-facts protocol 2 in
`solid-facts::compiler`. Producer structs stop in their dialect adapters. One
deep normalization module validates identity and semantics, then emits a
`CompilerSemanticModel`. The older `trackedRegions`, `untrackedRegions`,
`discardedRegions`, `ownershipRegions`, `callbackRoles`, and `jsxOperations`
arrays are deterministic compatibility views derived there. A decoded map is
refused if any view disagrees with the normalized operations.

## Solid 2 semantic trace version 3

The Solid 2 compiler emits two related operation sets.

- A source operation identifies one original-source span and semantic kind,
  such as a JSX child, native attribute, component property, event handler,
  ref factory, or control-flow render callback.
- A generated operation identifies a concrete lowering operation, such as an
  effect, insert, memo, scope, component invocation, deferred callback,
  delegated event, ref application, SSR claim, or runtime wrapper.

Source operations cite generated-operation IDs. Normalization verifies every
such reference, unique IDs, canonical span order, nonempty identities, and
canonical generated receiver spans. The producer reconciles censused source
sites with the decisions actually taken during lowering and refuses an
unresolved or conflicting site. Semantic recording can be disabled without
allocating facts and does not participate in lowering decisions.

The Solid 2 source-operation set is complete for supported lowering. Reported
generated operations are exact positive facts, but their set remains partial:
the producer does not yet maintain an independent census over every possible
generated wrapper emission. An empty or incomplete generated list therefore
never proves absence.

Every source operation carries independent execution axes:

| Axis | Meaning |
| --- | --- |
| disposition | whether the source is discarded, eager, deferred, reactive, event-triggered, a ref factory/application, a component getter/control-flow render, or SSR evaluation/render callback |
| trigger | no invocation, render, dependency, event, ref application, caller, or unknown |
| schedule | no execution, inline, render, deferred, or unknown |
| tracking | none, tracked, untracked, inherited, or unknown |
| cardinality | never, zero-or-one, exactly-once, zero-or-more, one-or-more, or unknown |
| owner | none, ambient at source, ambient at generated invocation, captured generated owner, created generated owner, or unknown |

The axes stay independent. An unknown owner does not erase known scheduling or
tracking. Validation rejects contradictory terminal combinations, including a
discarded operation that claims a live trigger, schedule, owner, or nonzero
cardinality. Generated operations carry trigger, schedule, tracking,
cardinality, and owner independently as well.

### Identity envelope

Trace version 3 binds facts to all inputs and products that can change their
meaning:

- compiler package version;
- exact official upstream base revision;
- exact semantic implementation revision;
- SHA-256 of original source bytes;
- SHA-256 of generated JavaScript;
- optional source-map SHA-256;
- generation mode and the complete effective compiler configuration, including
  filename and source-map selection.

The checker recomputes source, output, source-map, and canonical configuration
digests during normalization. It also requires the trace's compiler identity
to equal the constants exported by the pinned producer. A stale trace, changed
output, partial Solid 2 identity, or pin/schema disagreement fails closed.

The implementation revision deliberately names the first semantic
implementation commit. The following distribution commit changes only that
constant, avoiding an impossible self-hash. `rust/Cargo.toml` pins the exact
distribution commit and the Phase 4 report records both identities.

## DOM and SSR semantics

DOM and SSR are separately reconciled modes. Universal and dynamic generation
remain unsupported for semantic tracing and are refused.

DOM facts distinguish reactive reruns from eager values, deferred component
getters and children, events, control-flow render callbacks, discarded source,
ref factories, generated ref application, and owner-creating wrappers. In
particular, a two-phase ref/directive factory is not collapsed into its later
application.

SSR facts describe evaluation performed by the server transform and generated
render callbacks using inherited tracking. They are not projected to the old
"untracked render" array, because doing so would make a stronger and incorrect
client-style claim. Browser-only event and ref paths discarded by ordinary SSR
remain discarded; generated SSR render callbacks and claim operations have
their own identities. These facts do not claim request transport, response
commitment, stream flushing, or runtime package behavior.

## Server-function transformation facts

The compiler's `transformDirectives` interface has an independent,
output-neutral semantic trace for server-function transforms. It records exact
directive/source spans, module and function scope, export identity, whether a
server-function reference was created, and whether a server implementation was
registered. Its identity envelope binds source, output, optional source map,
configuration, and the same compiler revisions.

This is compiler transformation evidence only. It does not certify network
transport, serialization, authentication, deployment routing, or invocation
success. The ordinary JSX adapter does not invent those facts, and package
contract proof integration for the directive trace remains a later consumer
step.

## Solid 1 normalization

Solid 1 still emits semantic trace version 2. Its dialect adapter translates
legacy terminal decisions into the same protocol-2 source-operation model in
one place. Source-operation completeness is retained, but the old trace does
not bind generated wrapper observations to exact generated-operation IDs.
Therefore:

- `sourceOperationsComplete` is true;
- `generatedOperationsComplete` is false;
- the generated-operation list is empty and means unknown/partial, never
  proven absence; and
- producer `identityComplete` is false because the legacy trace lacks the full
  Solid 2 identity envelope.

This asymmetry is intentional. Shared consumers see one normalized model while
the older producer cannot certify facts it never emitted.

## Compatibility projection

Existing IR consumers continue to receive conservative projections:

| Normalized fact | Compatibility view |
| --- | --- |
| tracked reactive rerun | tracked region |
| discarded / cardinality never | discarded region |
| exactly-once untracked eager value or ref factory | untracked region |
| deferred/component getter | deferred callback |
| event-triggered | event-handler callback |
| ref application | directive-apply callback |
| control-flow render | render callback |
| reactive rerun under a created generated owner | owned region |
| SSR inherited execution, partial/unknown axes | no stronger legacy execution claim |

Every source operation still yields an exact JSX-operation kind. The source
AST independently detects JSX regions with no compiler census entry;
`missing_jsx_census` converts silence into an uncertifiable obligation rather
than negative proof. A positive discarded operation is different: it proves
the code executes zero times and dominates every live execution inference
inside its span.

## Proof obligations

A producer may mark a source or generated operation set complete only after:

1. every supported lowering entrypoint has participated in the census;
2. every censused source site has exactly one terminal semantic disposition;
3. speculative/retracted lowering observations have been rolled back or
   replaced with an explicit discarded fact;
4. every source-to-generated reference resolves uniquely;
5. every span and operation ID is canonical and deterministic;
6. the full compiler fixture corpus and adversarial probes reconcile;
7. trace-on and trace-off JavaScript, source maps, diagnostics, and observable
   compiler behavior are byte-identical; and
8. output without facts matches the exact upstream base.

The checker revalidates structural and semantic invariants. It cannot by itself
prove that a compromised producer observed every lowering branch, so closure is
accepted only for the revision whose independent reconciliation tests and
conformance report are pinned with the checker.

## Cache and protocol invalidation

Compiler cache keys include the source path and digest, dialect identity,
exact compiler-facts producer identity, protocol number, and canonical compiler
options. Reactive IR reuse compares normalized operations, completeness,
producer semantic revision, and compatibility views; it deliberately ignores
per-build source/output/configuration digests after normalization so a body-only
edit with identical execution semantics can reuse later indexes. Moving a
producer pin, trace version, semantic implementation, or protocol still
invalidates cached answers even when source semantics happen to match.

JSON modules use an explicit inert model with no producer operations because no
JSX compiler is invoked. An absent semantic model in a serialized protocol-2
map is a decode failure, not an empty fact set.

## Moving the pin

To adopt new Solid 2 compiler facts:

1. branch from an exact official `solidjs/solid#next` commit;
2. keep the fork delta semantic-only;
3. record separate semantic implementation and distribution commits;
4. update the Cargo revision, lockfile, trace-version assertion, adapter/cache
   identity, notices, and conformance report atomically; and
5. run compiler reconciliation/output-neutrality, both adapters, process tests,
   coverage, ownership, and full `make verify`.

No upstream Solid pull request is opened for the checker-only semantic trace.
A compiler behavior defect is fixed upstream in an independent contribution;
the semantic fork leaves the corresponding fact open until that fix is present
in its official upstream base.
