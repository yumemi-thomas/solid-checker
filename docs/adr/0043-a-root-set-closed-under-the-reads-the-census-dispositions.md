# ADR 0043: A root set closed under the reads the census dispositions

- Status: accepted and implemented (2026-09-06); written with the
  implementation
- Date: 2026-09-06
- Owners: Type Facts producer (`uncensused_invoking_forms.go`) and the policy-2
  `creates` census (`type_facts.rs`)
- Relation: the fifth slice of ADR 0034's premise, after ADRs 0040, 0041 and
  0042. Takes the case ADR 0041 deferred by name. Handshake protocol 26 → 27;
  lever G in `docs/package-contract-v2/accuracy-roadmap.md`.

## Context

ADR 0034 dispositions a form whose subject the producer roots at an unwritten
parameter, because the accessor it reaches was installed by the caller on an
object the caller passed. The **root set** it built for that was the parameter
list itself, filtered: a plain identifier binding, no initializer, no rest
token, unwritten, in a declaration mentioning neither `arguments` nor `eval`.

That set is narrower than the premise it serves, and the corpus says so. After
ADR 0042 the accessor class is 285 refusals at 72 sites, and the shapes that
dominate it are not values of unknown provenance — they are the caller's own
values, named:

```js
// motion-dom, projection/geometry/delta-remove.mjs — 18 refusals
function removeAxisDelta(axis, translate = 0, /*…*/ sourceAxis = axis) {
    const relativeProgress = mixNumber(sourceAxis.min, sourceAxis.max, /*…*/);

// motion-dom, projection/geometry/delta-apply.mjs
function applyBoxDelta(box, { x, y }) {
    applyAxisDelta(box.x, x.translate, x.scale, x.originPoint);

// motion-dom, render/html/utils/scrape-motion-values.mjs
function scrapeMotionValuesFromProps(props, prevProps, visualElement) {
    const style = props.style;
    // …
    if (isMotionValue(style[key])) {
```

`sourceAxis` holds the caller's argument or another parameter's. `x` is a
property of the caller's second argument. `style` is `props.style`, a chain
ADR 0034 *already* dispositions when it is written inline. Every one of them
refused, and the reason each refused was a spelling.

## Decision

**The root set is closed under the reads this census already dispositions.**
Naming an intermediate does not change whose value it is, so a form rooted at a
name that holds a caller's value takes the same disposition as the same form
written against the parameter directly. Four legs, and the fourth is a
different claim from the other three.

### The three that are ADR 0034 restated

- **A parameter's own object binding pattern.** `function f({ x })` binds a
  property of the caller's argument, exactly as `function f(a)` plus `a.x`
  reads one. Every plain identifier the pattern binds — and every one a nested
  object pattern under it binds, since that reads a property of a property of
  the same value — is rooted at that parameter, provided the parameter carries
  no default.
- **A local variable declaration whose initializer is rooted**, taken to a
  fixpoint so that `const a = p.x; const b = a.y;` roots both. The declaration
  may bind a plain identifier or an object pattern.
- Both carry the derivation `parameter`, because that is what they are.

These are not a widening of ADR 0034's premise; they are the same premise
reaching the same values. The check is direct: erase the intermediate and the
census already certifies the result.

**They also inherit ADR 0034's known limit rather than adding one.**
`const style = props.style` gives you whatever `props.style` holds, and if
*this module* had earlier written an accessor-bearing object onto the caller's
object, that accessor is this module's. That hole is exactly the one
`props.style.x` has had since ADR 0034, it is a `writes`-domain fact, and
naming the intermediate neither opens nor widens it.

### The one that is not

- **A defaulted parameter whose default expression is a reference to a
  parameter rooted the first way.** `sourceAxis = axis` holds the caller's
  argument at its own slot when one was passed and the caller's argument at
  `axis`'s slot when none was. Caller-supplied under either branch — but that
  is a claim about *two* branches, and ADR 0034's is a claim about one.

It therefore travels under its own derivation, `parameter-default`, and the
receipt's form site carries it, so a reader can tell the two apart and a
consumer that has reviewed only ADR 0034 can refuse the second.

### What the producer states

`subjectRoot` accompanies every stated `subjectParameter`, over a closed
two-value set, and the verifier refuses a form whose derivation it has not
reviewed — including an **absent** one, which is what a protocol-26 producer's
every rooted form decodes to. That is why the handshake moves although the
field is additive: reading a missing derivation as `parameter` is precisely the
absence-as-evidence this census exists to prevent.

## What the producer will not root

Each exclusion is a value that is not the caller's, or one this walk cannot
choose between:

- a **rest** parameter and a **rest** element — the array and the object are
  ones the engine built, which is the fact ADR 0042 already turns on. Not the
  caller's, so not rooted *here*; ADR 0044 roots an object rest element as a
  value this program built, and `patternRestParameter` certifies under that
  premise rather than this one;
- a binding element carrying **its own default**, and a parameter pattern
  carrying one — the default value is an object *this* code created, the case
  ADR 0034 excluded a defaulted parameter for and the case ADR 0041 named when
  it deferred parameter patterns;
- a default that is anything but a bare identifier, and a default naming a
  parameter that is **itself defaulted** — one reviewed hop, not a chain;
- a name bound by a `for…of` or `for…in` head, which has no initializer to root
  (ADR 0042 decides the callee case separately, and only for a call);
- a symbol with **more than one declaration**, whose running declaration this
  walk cannot choose — the same reasoning `census_local_binding_is_stable`
  applies to a callee;
- anything **written** anywhere in its file, at every leg. The premise is not
  flow-sensitive and says so.

## Alternatives considered

- **Reuse `parameter` for the defaulted leg.** Rejected: the receipt would then
  not distinguish a value the caller passed from one that may be a default, and
  the `writes` and `invalidates` censuses will need that distinction more than
  `creates` does.
- **Admit a primitive-literal default** (`translate = 0`), whose prototype
  chain is the default library. Sound, and it buys nothing measured — no such
  parameter is an accessor receiver in the corpus. Left for whoever measures a
  case.
- **Chain the defaulted leg** (`f(a, b = a, c = b)`). Equally sound by
  induction and equally unmeasured; one hop is what the corpus needs and one
  hop is what is reviewed.
- **Make the root walk flow-sensitive** so a written local qualifies between
  its writes. Rejected for the reason ADR 0034 gave: it needs a
  definite-assignment analysis and a new class of premise.
- **Root the *result* of a rest element or an object spread** as an
  engine-built plain object. Rejected here for the reason ADR 0041 gave when it
  named this as the next slice: it needs an escape premise, which is a flow
  question. Still the next slice.

## Consequences

- Handshake protocol 26 → 27; the schema digest and the Rust client move with
  it.
- `implementation-census-creates` gains eleven exports: `defaultedFromParameter`,
  `patternParameter`, `localBindingFromParameter` and `localPatternFromParameter`
  certify; `defaultedFromModuleValue`, `defaultedFromDefaulted`,
  `patternParameterDefault`, `patternElementDefault`, `patternRestParameter`,
  `localBindingWritten` and `localBindingFromCall` refuse (`patternRestParameter`
  until ADR 0044, which roots the rest object as this program's).
- **A trap the fixture records.** Three of those negatives first certified
  *vacuously*: written `source = { value: 1 }`, the compiler binds `value` as a
  data property of a literal in the same file, so the producer records no form
  at all and the census never reaches the premise. They sit on the untyped
  module value instead, the same guard `setterOnModuleValue` carries.
- Tests: `creates_census_dispositions_a_parameter_rooted_accessor_in_write_position`
  gains the derivation arm — the `parameter-default` site, an unreviewed
  spelling, and an absent one, all three;
  `the_probe_gate_tracer_census_closes_a_root_derived_by_naming_an_intermediate`
  and `the_probe_gate_tracer_census_stops_at_the_root_closures_boundary` run
  all eleven end to end, because the premise lives in the *producer* and a
  hand-written transcript would pin only the consumer's half.
  `TestUncensusedFormSubjectParameterIsStatedOnlyUnderTheParameterRootPremises`
  pins each leg's forms and each stated derivation.
