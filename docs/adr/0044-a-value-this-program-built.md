# ADR 0044: A value this program built

- Status: accepted and implemented (2026-09-07); written with the
  implementation
- Date: 2026-09-07
- Owners: Type Facts producer (`uncensused_invoking_forms.go`) and the policy-2
  `creates` census (`type_facts.rs`)
- Relation: the sixth slice of the lever ADR 0034 opened, and the first in it
  that is **not** about the caller. Takes the case ADR 0041 named as its next
  slice. Handshake protocol 27 → 28; lever G in
  `docs/package-contract-v2/accuracy-roadmap.md`.

## Context

ADRs 0034 and 0040–0043 all answer one question — *is the code this form can
reach the caller's?* — and each closed the shapes where it is. What is left of
the accessor class after ADR 0043 is 98 property reads and 103 element reads
whose receiver is not the caller's at all. It is this module's:

```js
// motion-dom, projection/styles/scale-correction.mjs — and five more like it
const scaleCorrectors = { borderRadius: { …correctBorderRadius }, /* … */ };
function isForcedMotionValue(key, { layout, layoutId }) {
    return … (!!scaleCorrectors[key] || key === "opacity");

// motion-dom, render/html/utils/build-transform.mjs
const transformPropOrder = ["transformPerspective", "x", "y", /* … */];
    const key = transformPropOrder[i];

// motion-dom, animation/utils/resolve-transition.mjs — ADR 0041's own example
const { inherit: _, ...rest } = transition;
return { ...parentTransition, ...rest };
```

**The census already takes this premise — silently.** `scaleCorrectors.opacity`
records no form at all: the compiler binds the member to a `PropertyAssignment`
in this file, `accessorKindForSymbolLocked` sees no accessor declaration, and
the form is never emitted. Only the *computed* key refuses, and it refuses
because the checker resolves no symbol for it, not because anything about the
object changed. The difference between a certified read and a refused one was
the syntax of the key.

## Decision

**A binding this program initialized from an object or array literal, or from
an object pattern's rest element, has only data properties, and a form whose
subject is a direct reference to such a binding is dispositioned.**

The premise is the specification's, not an approximation of it. An object
literal creates every one of its members with `CreateDataPropertyOrThrow`; so
does a spread member inside it, through `CopyDataProperties`; so does an array
literal, by index, a spread element included; and so does the object a rest
element binds. Reading any member of such a value therefore reaches a data
property, or the prototype chain — `Object.prototype` or `Array.prototype`,
whose one accessor (`__proto__`) is the engine's. Writing one sets a data
property or reaches that same chain.

It travels as the derivation `own-literal`, with **no** `subjectParameter` —
this is not a claim about the caller — and with `subjectDeclaration`, the
binding's exact range, which the consumer places in the analyzed artifact's own
runtime source itself before reading the premise. The receipt records
`own-literal-accessor`, `own-literal-accessor-write` or `own-literal-iterable`,
so a reader can tell a value this module built from one its caller passed.

### Through an import

The shape that dominates the class is a table declared in one module and read
in another — `transformPropOrder` lives in `keys-transform.mjs` and is read in
`build-transform.mjs`. At the use site the identifier binds an **import**, so
the producer resolves the alias to the exported binding and asks the questions
above of *that* declaration, in *its* file: is the initializer a data-only
literal, is the symbol declared once, does anything in the declaring module
write it. The location it states is the declaration's, so the consumer's
runtime-source check is asked of the declaring module too, which is what keeps
a table re-exported from a dependency out. A first draft of this ADR scanned
only the censusing file and closed a third of what it should have; the
measurement caught it.

### A direct reference only

`table[key]` is dispositioned; `table[key].member` is not. What a data property
*holds* is an arbitrary value, so the outer read is a second question this
premise does not answer — and the two are separate forms, so the inner one
clears while the outer refuses. That is why the root walk, which follows a
receiver chain for a parameter, takes an own literal only at the head. For the
same reason an own literal does not propagate through a local binding:
`const item = table[key]` names a value nothing here speaks for.

## The hole, stated, and why it is not a new one

Nothing stops a third party from calling
`Object.defineProperty(scaleCorrectors, k, { get() { … } })` on an exported
table, and this census would not see it.

That hole is **already taken**, and taken more quietly: it is exactly what the
absence of a form for `scaleCorrectors.opacity` asserts today, and has asserted
since the accessor classifier was written. This ADR does not open it; it makes
one premise explicit and receipt-carrying where the same premise was implicit
in a missing row. If it is ever to be closed, it must be closed for the literal
key first — and this ADR is what makes that possible to find, because the
premise is now written down and named in every receipt that rests on it.

## What the producer will not root

- An initializer that is **anything but** an object or array literal — a call,
  a conditional, `new`, another identifier. `const newValues =
  scrapeMotionValues(…)` returns a fresh object, and proving *that* is a
  question about the callee's return, not about this binding.
- An object literal that installs a `get`/`set` member, whose accessor is this
  module's own code and is exactly what the premise excludes.
- An object literal carrying a `__proto__:` member: that sets the prototype
  rather than a property, replacing the one chain this premise reasons about.
- An **array pattern's** rest element. Its elements come from the source's
  iterator, which is the source's code — an iteration question (ADR 0042), not
  a property one.
- A binding **written** anywhere in its file, or whose symbol has more than one
  declaration.
- Any subject that is not a direct reference to the binding.

## Alternatives considered

- **An escape analysis** — admit a local literal only when nothing in the frame
  passes it anywhere between its creation and the read. Rejected as the wrong
  shape for the actual hole: it would not help the module-level tables that
  dominate the class (they are exported), and for the locals it would refuse
  cases the literal-key path already certifies, leaving the census
  self-inconsistent.
- **Reuse `subjectParameter` with a sentinel.** Rejected: the field means a
  parameter of the transcript's own declaration, and there is no parameter
  here. A derivation with a different companion fact is the honest encoding,
  and the consumer refuses either fact appearing beside the wrong derivation.
- **Trust the producer's derivation alone**, without the declaration location.
  Rejected: this is the first derivation naming a binding the consumer does not
  already hold, and placing it in the artifact's own runtime source is cheap
  and rules out a literal in a declaration file or a dependency.
- **Admit the return of a reviewed array-building default-library member**
  (`split`, `match`, `filter`). Sound by the same specification argument and
  measured as the next tranche (`match[2]`, `value.split("/*")[0]`,
  `resolvedKeyframes[index]`); left out because it is a table of members to
  review, which deserves its own ADR the way the invoker table did.

## Consequences

- Handshake protocol 27 → 28; the schema digest and the Rust client move with
  it.
- Measured on the 2026-09-07 corpus: withheld candidates 662 → 631,
  `censusRefused` 605 → 574, the element-access class 103 → 46 and the spread
  class 41 → 29. **Thirty-one candidates.** Twelve of them came only once the
  alias was followed: a first draft that scanned the censusing file alone
  measured 643, because the tables that dominate the class are imported.
  Statuses unchanged, 368 certified / 30 refused.
- `implementation-census-creates` gains nine exports: `ownTableRead`,
  `ownArrayRead`, `ownTableWrite` and `ownRestSpread` certify;
  `accessorTableRead`, `protoTableRead`, `writtenTableRead`,
  `ownTableMemberRead` and `arrayRestRead` refuse. ADR 0043's
  `patternRestParameter` flips from refusing to certifying: a *parameter*
  pattern's rest element is built by the same CopyDataProperties, and the
  reason ADR 0043 would not root it at the caller is the reason this ADR roots
  it as the program's. `make verify` is what surfaced the flip.
- The fixture's `key` and `index` parameters must stay `any` in `index.d.ts`.
  A literal key would bind a data property and record no form, which is the
  vacuity ADR 0043's README note already warns about — and here it is the whole
  point of the ADR, so a vacuous fixture would pin nothing at all.
- Tests: `creates_census_dispositions_a_read_of_a_value_this_program_built`
  pins both positions and all three refusals of the stated shape (no
  declaration, a declaration outside runtime source, a parameter index beside
  the derivation);
  `the_probe_gate_tracer_census_closes_a_read_of_a_value_this_program_built`
  and `the_probe_gate_tracer_census_stops_at_the_own_literals_boundary` run all
  nine end to end.
