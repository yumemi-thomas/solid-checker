# class-creator-caller-creates

The soundness pin for the `new`-site gap ADR 0158 records: a module-private
class whose constructor creates (`createEffect(read, () => {})`), constructed
by `createThing` and reached by `wrapThing` only through `createThing`, the
shape of `@tanstack/solid-router`'s `createFileRoute` → `createRoute` →
`new Route`. `plain` is the control.

**What the generator does** (`expected.json`): it proposes `creates: []` for
all three exports. Its `creates` walk (`rust/crates/solid-reactive-ir/src/creates_walk.rs`)
follows calls into project *functions* only, and `new Thing(read)` resolves to
the class's constructor declaration, which it never enters. The direct call
`createEffect(read, () => {})` in an ordinary function declines
`dialect-silent`; in the constructor it declines nothing. That is an
over-proposal, not a claim.

**What certification does** (the tracer
`contract_certification::tests::a_creating_constructor_withholds_creates_from_every_export_that_reaches_it`,
planned from this `expected.json` byte for byte, integrity rebound): the
`creates` census walks the construction into the constructor, for
`createThing` directly and for `wrapThing` through `createThing`, and
withholds both closures by name at the constructor's `createEffect` call
(`census refused: … (\`createEffect(read, () => {})\`)`). Neither export can
certify `creates` closed. `plain`'s census passes. Its closure is withheld by
the veto only because the probe worker cannot import `solid-js` in a
transaction that carries no `solid-js` archive.

The `solid-js` stub is `render-effect-apply-callback`'s, byte-faithful to
`solid-js@2.0.0-rc.3` for `createEffect` (its header lists the sources).
Nothing here produces a finding, so no `tsc` case applies.
