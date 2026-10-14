# ADR 0309: A reactive value's flow is one fact

- Status: accepted (owner, 2026-10-14), with the ten decisions below. Not
  started. Design pass: `rust/target/research/adr0309-design.md`.
- Owner: a new `solid-reactive-ir` stage, `value_flow`, between the project
  indexes and `interproc`.
- Phase 1b of docs/contract-removal-plan.md. It comes before phase P2.

## Context

The analyzer has no single answer to one question: where can a proven
reactive value go, and who can call it? About ten modules each answer part
of it, for the shapes they know:

- `accessor_provenance` (exact project callable transport);
- `value_identity` (invalidation of mutable bindings);
- `object_members` (ADR 0299, calls through immutable object bindings);
- `argument_values` (ADR 0280, what arguments prove about parameters);
- `property_gets` (ADRs 0274 and 0283, Gets at a package call);
- the escape helpers in `local_access`, `owners` and `interproc`;
- `reactive_handoff` (ADR 0307, and the allowlist on the
  `p1b-kobalte-namespace` branch).

Each slice certifies what it follows and says nothing about the rest.
Silence then reads as "clean." Phase 1 review rounds found this twice:

- Round 1: `access.call`/`apply`, `accessArray`/`chain` and `withAccess`.
- Round 2: a parameter default initializer, spread into a call, a tagged
  template, `new Proxy(value, {})`, `Array.from(xs, value)`,
  `xs.forEach(value)`, and a JSX prop called in the child's body.

The named-import or project-local form of every one of these was
certified at `d9ed2224d`, although the runtime reads the signal untracked. Each fix closes the shapes found. The
next round finds new ones, because every shape falls between two slices. An
allowlist over the references (P1b round 3) stops the wrong certifications
by refusing. It cannot prove anything new.

The same missing fact blocks recall that contracts provide today and source
analysis does not:

- probus's `@tanstack/virtual-core` constructor: a fresh `this`, a
  class-field arrow `setOptions`, and option getters read through a spread;
- `Object.assign` and `defineProperties` over static data properties;
- `toObservable`, whose only call to the accessor sits in a tracked effect;
- kobalte's `Root → Polymorphic → Dynamic` prop path.

## Decision

One fact records the flow of each proven reactive value: an accessor, a
store or a reactive prop getter, as the dialect's vocabulary and the
existing provenance establish them. Shapes and declared types are never
evidence (ADR 0294).

**Nodes** are abstract allocation sites:
- function values and closures;
- object, array and class-instance allocations (`new C`, with constructor
  order: field initializers, then the body);
- namespace objects;
- bindings, parameters (including defaults and rest), returns, and exact
  static properties.

A container whose index or key is not static has one summarized element.

**Edges** come from the exact symbol references of the binder and the
normalized calls. They cover every carrier: aliases, spread elements,
template substitutions, default initializers, JSX attribute values, `Proxy`
targets, callback arguments, `call`/`apply`/`bind`, returns, property writes
and container stores.

**Read events** are where a value can be read: calls, and the Gets and
protocol steps that run code. These are getter Gets, iteration, a `then`
lookup, a spread, a `for…in` enumeration, and a tagged template call. Each
event carries an invocation context from the existing facts: synchronous on
the current stack under a tracked or an untracked context, or deferred past
the stack.

**Terminals** are where a flow ends:
- **Read events,** as above.
- **Known non-invoking operations,** each an exact entry in one table, for
  example `Object.assign` copying a data property. The flow continues
  through the copy.
- **Unknown,** the escape. This covers an unresolved callee, a computed key,
  a reflective or prototype write, an unselected package, and a host API
  without a model. The ADR 0300 integrity premise limits which unseen code
  can write.

**Query:** a value is certified only when every flow ends at a read event
whose read has a row, or at a non-invoking terminal whose output flows are
certified in turn. A flow that reaches unknown is an SC9011 obligation that
names the first escape. It is never a silence.

**Seeds** are proven reactive values only, followed forward. A flow enters
package source only for selected packages (ADRs 0291, 0304). The cost
therefore follows demand, not program size. Budget: at most 1.2× each
project's corpus wall time.

## Owner decisions (2026-10-14)

1. **Shared flow evidence, not an execution certificate.** The fact supplies
   transport and read events. The existing source, completion, execution and
   owner proofs keep their own owners and query it.
2. **Stored or unused values.** Storage is a transfer, not proof. A
   published or opaque retained value is unknown. Certification needs
   evidence that the value is used completely, or that nothing at runtime
   consumes it.
3. **Captured reads under unknown invocation.** A strict read inside a
   closure that unknown code invokes is closed only by a closed census of
   the captured uses, where every actual invocation installs a proven safe
   read context. Other claim domains stay open. This admits `toObservable`
   without trusting names.
4. **Context sensitivity.** Relational summaries over actuals, formals,
   results and captures, with bounded invocation specialization that keeps
   per-instance source pairing. Overflowing the bound refuses explicitly. A
   member of a may-set never yields a violation.
5. **Mutation premise.** ADRs 0273 and 0300 stay, as explicitly tagged
   evidence. An exact allocation write discharges a veto only with a
   complete derivation of its target. Dynamic reflection, prototype writes
   and `eval` stay unknown. An assumed absence of writes never becomes an
   affirmative census.
6. **Legacy fixtures.** Old clean fixtures are not authoritative where an
   unsound table fallback produced them. Reviewed legacy behaviour is pinned,
   but unknown standard-library, bodyless and JSX exemptions are blocked.
   tsc-clean runtime controls precede any intentional conservative snapshot
   change.
7. **Escape location.** An escape is reported at the current seed hand-off
   location, with the exact first unknown operation as evidence. "First"
   means the shortest transfer path, then canonical source order, never
   worklist or hash order. Every unknown is retained internally, and a
   cleared path never hides another one.
8. **Cost baseline.** Two comparisons: against P0, and paired against the
   build after the round-3 allowlist. The limit is 1.2× per project, shadow
   overhead and peak memory included. The benchmark runs before any consumer
   moves.
9. **Constructors.** The first recall commit covers exact selected base
   classes, with an affirmative `this` and field-initializer order. Derived
   classes, constructors that return a replacement, and auto-accessors
   refuse until they are normalized and proven separately. No
   package-specific name tests.
10. **External boundaries.** Typed-candidate obligations and unknown
    package and prop surfaces stay outside the graph. Seeding cannot replace
    SC9005/SC9012 discovery for unproven external inputs, and a type never
    creates authority (ADR 0294).

## Migration

The design pass section 7 is the commit order: the round-3 allowlist first;
then the fact in shadow mode (identities, missing fact events, edges, read
events), no finding moving; then one consumer per commit; then deletions;
then one recall target per commit.

1. Build the fact beside the existing slices, in shadow mode. On every
   fixture and corpus project it must be at least as strict as the P1b
   allowlist: a value the allowlist refuses is either refused or proven with
   a read row. A divergence blocks the build.
2. Move consumers one at a time: `reactive_handoff`, then `property_gets`,
   `object_members` and the transport half of `accessor_provenance`. Each
   move keeps snapshots byte-identical, or explains every moved finding.
3. Delete each slice once nothing consumes it.
4. Recall then comes from the fact, not from new shape cases: the
   virtual-core constructor, `Object.assign` data properties,
   `toObservable`, then kobalte `Dynamic`. Each gets its own fixtures and
   review round.

## Consequences

- A review attacks one fact and one terminal table, not an open list of
  shapes.
- Certification no longer depends on which slice happened to follow a
  value. The same runtime behavior gets the same verdict with or without a
  project helper in between, which review round 2 showed fails today.
- The P1 recall goals become consequences of the fact, and they are no
  longer phase 1 exit criteria.
- It is a large change across `interproc` and its neighbours. Its own
  review rounds and the shadow-mode gate are the safeguards.

## Non-goals

- No name matching, guessed dispatch or structural signatures.
- No type-only provenance.
- It does not replace `local_access`'s classification of reads by execution
  role. The fact consumes that classification.
