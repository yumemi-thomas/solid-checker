# ADR 0041: A spread's operand and an object pattern's source are subjects too

- Status: accepted and implemented (2026-09-06); written with the
  implementation
- Date: 2026-09-06
- Owners: Type Facts producer (`uncensused_invoking_forms.go`) and the policy-2
  `creates` census (`type_facts.rs`)
- Relation: the third slice of ADR 0034's premise, after ADR 0040 took the
  second. Handshake protocol 24 → 25; lever G in
  `docs/package-contract-v2/accuracy-roadmap.md`.

## Context

ADR 0034 dispositions a property or element access whose receiver the producer
roots at an unwritten parameter, because the getter or trap it reaches was
installed by the caller on an object the caller passed. ADR 0040 extended that
to write position, on the ground that the premise turns on the provenance of
the code rather than on reading.

Two forms that read properties of exactly such a value were still refused,
because the producer roots a subject only for an access node. On the corpus
after ADR 0040 they are 73 of the 284 remaining accessor refusals:

```js
function combineStyle(a, b) { return { ...a, ...b }; }        // SpreadAssignment
function buildHTMLStyles(state, latestValues, transformTemplate) {
    const { style, vars, transformOrigin } = state;            // BindingElement
```

An object spread reads every own enumerable property of its operand and invokes
each getter among them. A binding element of an object pattern reads one named
property of the destructured value, and a rest element reads whatever own
properties remain of it. In both cases the value is the one the caller handed
to this invocation, and every accessor reached sits on that value.

## Decision

**An object or JSX prop spread whose operand is rooted at an unwritten
parameter, and a binding element of an object pattern whose destructured source
is so rooted, take the same disposition as a named read.**

Nothing about the premise changes. What changes is which expression the
producer treats as the form's *subject*:

| form | subject |
| --- | --- |
| property / element access | the receiver |
| object or JSX prop spread | the spread operand |
| binding element of an object pattern | the value the **outermost** enclosing pattern destructures |

Taking the outermost pattern is what makes a nested pattern correct rather than
merely permitted: `const { a: { b } } = src` reads a property of `src` and then
a property of that, which is the same chain of reads ADR 0034 already follows
through a receiver chain, rooted at the same parameter. A rest element takes
the same subject, because the properties it reads are the source's.

Both forms are reads, so neither records ADR 0040's write disposition.

### What the producer will not root

A binding pattern in **parameter position** — `function f({ style })` —
destructures the caller's argument directly, which is the same provenance by a
shorter route. It is deliberately left unstated here, because a parameter
pattern also admits a default (`function f({ a } = {})`) whose object *this*
code created, which is the case ADR 0034 excludes a defaulted parameter for.
Deciding it needs the default distinguished, and that is a separate premise.

Only a variable declaration with an initializer supplies a pattern's subject
today; a destructuring assignment (`({ a } = src)`) reaches the census through
different node kinds and is not addressed.

## Alternatives considered

- **Root the spread by its own node's receiver.** There is no receiver: a
  spread names one operand, and reading that operand's properties is the whole
  form. The operand is the subject.
- **Root each binding element by its own nearest pattern.** Rejected: a nested
  pattern's nearest source is a synthetic intermediate, not an expression the
  root walk can follow. The outermost pattern names the one expression that
  exists.
- **Admit a parameter pattern immediately.** Rejected above: the default case
  is a real hole and deserves the same care ADR 0034 gave defaulted parameters.
- **Treat the result of a spread or rest element as a plain object** — its own
  properties are data properties, so reading it invokes nothing. True, and it
  would close `{ ...parentTransition, ...rest }` where `rest` is a local the
  engine just built. Rejected here because it needs a premise that nothing
  installed an accessor on that local between its creation and the read, which
  is a flow question this census does not answer. Recorded as the next slice.

## What still refuses

- **A spread of a written parameter.** `combineStyle(a, b)` assigns to `b`
  before spreading it, so `...b` refuses and the export stays withheld even
  though `...a` clears. The root premise is not flow-sensitive, and says so.
  Pinned by `spreadWrittenParameter`.
- **A spread or destructuring of anything not parameter-rooted** — a module
  value, a call result, a local the engine built (`...rest`). Pinned by
  `destructureModuleValue`.
- **A binding pattern in parameter position**, per the decision above.
- **An array pattern's elements**: the iteration it performs is recorded as
  `iteration-protocol` on the pattern itself and is a different reach.
- Every other form ADR 0034 left refusing.

## Consequences

- Handshake protocol 24 → 25; the schema digest and the Rust client move with
  it.
- `implementation-census-creates` gains `spreadParameter` and
  `destructureParameter`, which certify, and `spreadWrittenParameter` and
  `destructureModuleValue`, which refuse.
- Measured on the 2026-09-06 corpus: withheld candidates 745 → 739, and
  the accessor class 284 → 277. The **form** counts move much further than the
  candidate count: `BindingElement` falls 42 → 6, while `SpreadAssignment`
  rises 31 → 41 and the two access kinds rise by 19 between them, because a
  body that used to refuse at its destructuring now reaches its next form. Six
  candidates is therefore the honest gain, and the measurement names the slice
  that would collect the rest: `resolveTransition` clears
  `const { inherit: _, ...rest } = transition` and then refuses at
  `{ ...parentTransition, ...rest }`, where `rest` is the plain object the
  engine just built. Statuses unchanged, 368 certified / 30 refused.
- Test: `creates_census_dispositions_a_parameter_rooted_spread_and_pattern`
  pins all three node kinds and the read disposition;
  `creates_census_parameter_rooted_accessor_stops_at_its_stated_boundary` keeps
  an unrooted spread and an unrooted binding element refusing, so the subject
  stays the premise rather than the node kind.
