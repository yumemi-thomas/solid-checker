# An arrow-bound export is an export at every rung of the ladder

`export const Arrowed = props => ...` has no `FunctionFact.name` and no
`method_name`; its only name is the binding that initializes it. Attribution
used to read those two fields alone, so an arrow export was *unnameable* — and
the rung that consumes the call graph turned "I cannot name this reaching
function" into "it is not an export", returned an empty enumeration, and marked
nothing. The documented fail-closed answer for an unanswerable question was
never reached.

The three marked exports cover the two rungs that were blind:

- `Direct` is the joined rung: the unresolved `.getThing` dispatch is written
  inside the arrow's own body.
- `Arrowed` is the reachability rung: the obligation is inside the private
  `Panel`, and only the call graph can say which exports enter it. `Arrowed`
  is an arrow; `Declared` is the byte-identical declaration form beside it, so
  a regression that resurrects the name-field-only lookup shows up as a
  difference between two exports that must be treated the same.
- `Isolated` reaches neither helper and stays certified. It is what makes the
  fixture a proof rather than a smoke test: widening everything to unknown
  would also "fix" the arrow, and would be caught here.

Both exported helpers carry the same claim, so both `Arrowed` and `Declared`
are marked in exactly the two domains an unresolved dispatch invalidates:
`reactiveReads`, because that is the summary the obligation says is unproven,
and `returns`, because the returned property is described from the local
accessor index, which knows nothing about the dispatch's result.

The `because` note on each `unknown-sentinel` item of `<contract>.review.json`
records which rung answered — `joined` for `Direct`, `reachability` for the
other two.
