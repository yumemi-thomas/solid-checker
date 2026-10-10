# ADR 0104: a property read of a dependency's data object

Status: accepted (2026-09-14). Handshake protocol 58. The census arm is proven
on the real corpus artifact — `createHydratableSignal` moves from `census
refused` to decidable — and the rows do **not** close yet: an empty `reads`
enumeration takes no synthesized veto, so the two exports need hand recipes.
See "What still withholds these rows".

## The shape

~~~js
// @solid-primitives/utils@6.4.1, dist/index.js
import { getOwner, onCleanup, createSignal, untrack, sharedConfig, onMount, DEV, equalFn } from "solid-js";

export function createHydratableSignal(serverValue, update, options) {
    if (isServer) return createSignal(serverValue, options);
    if (sharedConfig.context) {                       // :166
        const [state, setState] = createSignal(serverValue, options);
        onMount(() => setState(() => update()));
        return [state, setState];
    }
    return createSignal(update(), options);
}
~~~

The `reads` census refuses on one node:

> `reads-census premise required: the property-access-unknown-accessor form
> (PropertyAccessExpression) … states no reviewed subject root, so whose value
> it reads is undecided`

`sharedConfig` is an imported binding, so the producer refuses it as
`imported-binding`: whatever it holds was built elsewhere, and rooting it is a
cross-module question.

Measured on the 2026-09-14 pin: 124 recipe-less `reads` rows —
`createHydratableSignal` 62 and `createHydrateSignal` 62, the second being
`export const createHydrateSignal = createHydratableSignal`.

## It is exactly one node, and that was measured before building

The census reports only the *first* refusal, so "lift this and the export
closes" was a guess worth checking. A focused producer test over the same
source shapes answers it: the transcript states **one** uncensused invoking
form, the `sharedConfig.context` access, and is otherwise `complete` with no
open reasons.

Two things that looked like risks are not:

- **The caller-supplied `update()`** states no form at all. ADR 0034 already
  owns it — a callable the caller passed is analyzed in the caller's artifact.
- **The `createSignal` and `onMount` calls** state no form either, and the
  four sibling exports that reach `solid-js` primitives (`createSharedRoot`,
  `createSingletonRoot`, `createBranch`, `createDisposable`) all measured
  decidable in the pass-2 census.

## What the producer states

`SubjectRootDependencyMember` — `"dependency-member"` — with
`ImportedModuleMember { specifier, name }`, when the receiver of the access is
an identifier whose raw symbol (before alias resolution) is an import
specifier, never assigned in this file, from a **bare** module specifier.

It states the *exporting module's* name, not the local alias: `import
{ sharedConfig as config }` states `sharedConfig`, because that is what a
reviewer reviews.

Four shapes refuse, each for its own reason rather than by omission:

- **A namespace import.** `solid.sharedConfig` reads the module namespace
  object, whose properties the specification installs as accessors, and which
  member it lands on is a second question on top of the one being asked.
- **A default import.** What a module's default export holds is decided by the
  exporting module's own expression, and `default` does not name a reviewable
  member the way a named export does.
- **A relative or absolute specifier.** It names a file of this same artifact,
  whose census already walks it. Handing a consumer `"./util.js"` invites it to
  match a reviewed table entry against a path.
- **A locally assigned binding.** An import binding cannot be assigned in
  conforming code; the check is cheap and the census does not rely on the
  program being conforming where it can ask.

## What the certifier decides

The producer is looking at the *importing* module, whose source says nothing
about what the exporting one built. So it states identity and stops, exactly as
ADR 0103 does for a default-library alias, and
`REVIEWED_DEPENDENCY_MEMBERS` — keyed `specifier:name` — carries the reviewed
answer. A member earns an entry only when all three answer yes against the
audited version of that dependency:

1. Is the export an object that dependency's own module evaluation built,
   rather than one it received or built from caller input?
2. Are its own properties data properties throughout its lifetime — no accessor
   installed at construction, none installed later by the dependency itself?
3. Is reading one free of reactive effect — not a signal read, not a
   subscription, nothing a `reads` closure would have to enumerate?

`solid-js`'s `sharedConfig` is the hydration context object: a module-level
object literal whose properties the runtime assigns and reads as plain data,
and reading `context` subscribes to nothing.

A write *through* the receiver is refused. That is this artifact acting on
someone else's object, a different claim, and one no ADR has reviewed. Only
`property-access-unknown-accessor` is admitted — not an iteration protocol, not
an `instanceof`.

## The exposure this records rather than hides

An application can reach the same module singleton and install an accessor on
it — `Object.defineProperty` on an imported object is forbidden by nothing —
and no fact here would see that. This is the same exposure ADR 0044 already
carries for a literal the artifact exports, and it is exactly why the table is
keyed by a reviewed pair rather than admitting imported objects as a class.

## What still withholds these rows

The census stops refusing; the row is then withheld for want of its **mandatory
veto**, and no synthesized veto can serve it. `candidate_observation` registers
nothing for an empty `reads` enumeration, and says why: the contradiction of
`reads: []` is a read of a source the export *owns*, which no synthesized
module can instrument
(`2026-09-10-reads-veto-observation-design.md` § 6).

So these 124 rows close the way Tier A's 358 did — with two hand recipes, one
per export. That is authoring, not a premise, and it is deliberately not
bundled into this ADR: the producer fact and the reviewed table are what a
reviewer has to agree to here, and they are complete and measurable on their
own.

**Do not read this ADR as having closed 124 rows.** It has moved them from
`census refused` to decidable, which is the half a recipe cannot do for itself.
