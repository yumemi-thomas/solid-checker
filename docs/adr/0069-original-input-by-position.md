# Original input by position, not by prefix

ADRs 0065–0067 answered "does this read see the caller's value" by walking a
contiguous opening prefix of `const` declarations and stopping at the first
other statement. Three of the corpus's remaining refusals do not have such a
prefix at all. Kobalte Utils `scrollIntoViewport` opens with an `if`, computes
a guard from `window.getComputedStyle(...)`, and reads `targetElement` inside
the guarded branch; the only assignment to that parameter is in the *other*
branch, inside a `while` loop it never enters on that path.

The prefix was answering the wrong question. `initialParameterReadsLocked`
already refuses every implementation containing a nested callable, `arguments`
or `eval`, and every parameter that is defaulted, rest, destructured or
duplicately named. In what survives, no code outside the body can reach these
lexical bindings, so the only writer is a direct assignment in this body, and
the question is one of order alone: can any such assignment's store run before
this read?

It cannot when the read ends at or before the store, and no iteration statement
encloses both. A loop is the only construct that runs a later position before an
earlier one: a call cannot re-enter this activation's bindings, and `break`,
`continue`, `switch` and `throw` never jump backwards. So the cutoff for one
write is the start of the outermost iteration statement enclosing it, or — when
none does — the end of a plain `=` (whose entire right-hand side evaluates
before the store) and the *start* of every other write form, because a compound
assignment, `++`/`--` and a `for…in`/`for…of` head all read or store within
their own extent.

Protocol 43 marks each such row `positional`. The mark is not decoration: a
prefix row says *some* read of the slot saw the caller's value, which is what
the whole-root arm accepts with `|_| true`; a positional row says *this* read
did. Only `initial_parameter_member_identity`, which binds the use to the exact
call of the operation it is discharging, may take one. The whole-root arm keeps
the prefix and first-receiver rows only, so the additional rows cannot widen a
premise that was never tied to its operation. A row may not claim both
`positional` and `firstIterationOnly`; the loop that admits the second is
exactly what the first refuses. The producer states the mark, the Rust client
validates it against the same use census, and the verifier records a
`positional-parameter-input` witness distinct from the other two.

This is not a reachability claim, and it deliberately does not become one. The
producer's use reachability is optimistic — a read inside an `if` is reported
`Reachable` — so a marker derived from it would have said nothing. It does not
need to: a branch read has no later-value problem to guard against, which is
what separates it from `firstIterationOnly`, where the same read observes a
written value on the second entry. Whether the operation occurs at all remains
the operation's own lower bound and floor, proved elsewhere and unchanged here.

Three producer regressions that the prefix rule pinned at zero now bind one row
each, and each is the ADR's point rather than a side effect: a read inside a
branch before a later write, a read after a preceding `console.log()`, and a
read in a call *argument* on the right-hand side of the assignment that writes
the slot. New cases pin the branch-exclusive shape and a read before a writing
loop as admitted, and pin as refused a read after a branch write, a read after a
writing loop, a later `switch` case, an `++` write, and — the ADR 0050 shape
that had been reported unwritten — a destructuring assignment before the read.
The existing loop, alias, closure, `arguments`, `eval`, default, rest, computed
and same-statement refusals are unchanged.

No receipt, contract, trust or proof format changes. A missing row still confers
nothing, and a parameter this body never assigns is absent from the rule
entirely: `unwrittenParameterBindingsLocked` states that stronger fact instead.

Measured, and kept on the ADR 0049 grounds. The producer states five positional
reads for `scrollIntoViewport` and the corpus does not move by one row: the
demands this shape raises take the whole-root arm, which cannot bind a use to
its operation because `Operation` has no source location, so it quantifies over
all reads of the slot and no positional row may answer it. A diagnostic build
that lets one through certifies the package outright — 19 to 23 entrypoints with
the root — which is precisely why the gate stays: the arm's inference is
existence-quantified and invalid in general, for prefix rows as much as these.
The next fact is not another origin premise but an operation input's own site.
The [measurement](../package-contract-v2/phase21/2026-09-08-positional-origin-measurement.md)
records the probes, the diagnostic, and the two counterexamples that do not
settle it.
