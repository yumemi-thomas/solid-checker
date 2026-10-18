# ADR 0321: An argument literal's function property is its implementation

- Status: accepted and implemented (2026-10-18).
- Owners: `literal_property_function_symbol` and
  `is_literal_property_function_symbol` (`solid-reactive-ir/src/interproc.rs`),
  `member_value_candidates` and `member_value_symbols_at_tracked`
  (`solid-reactive-ir/src/indexes.rs`).
- Relation: completes ADR 0320's member resolution for function literals;
  narrows its name census for a literal written as the argument itself.

## Context

`useThing({ onClose: () => … })` with `function useThing(o) { o.onClose() }`
asks which `onClose` runs. ADR 0320 resolves a literal's members to symbols
and compares the summaries behind them. A method (`onClose() {}`) has its
method symbol and a summary node. An arrow or `function` expression written as
the property's value has neither: TypeScript gives it no symbol and it is not
in the summary-node universe, so the dispatch stayed unresolved
(`parameter-member-target-unresolved`) or diverged (the key's symbol with no
node). Measured on the 49-project corpus (`/tmp/sweep-final13.json`): 93
member-dispatch obligations had exactly such a literal behind them, 89 of
them at a callee that keeps its parameter closed.

ADR 0320 also refused every literal whose member name some code assigns
anywhere (`x.onClose = …`), because such a write may reach the object.

## Decision

1. **Identity.** A function literal (arrow or `function` expression, not a
   method) that is exactly the value of a non-computed, static-keyed,
   non-shorthand property of an object literal written directly as a call's
   argument has the identity `literal-property:<file>:<start>:<end>`. A literal
   returned, bound to a name or stored keeps the property key's symbol, as
   before: member calls on its holder name that symbol.
2. **Resolution.** `member_value_candidates` returns that identity for such a
   property. The literal's other refusals stay: a later spread, a getter, a
   computed or `__proto__` key, a `function` in the literal that uses its
   receiver.
3. **Summary.** The literal's reads stay attributed where it is written, as a
   callback's, by the read rules and ADR 0315/0319's flow for the literal. At
   the dispatch it contributes no reads (an empty summary): the dispatch stops
   being unresolved and claims nothing new. Two literal alternatives compare as
   equal; a literal against a named function compares by the named function's
   reads.
4. **Freshness.** A literal written as the argument itself is held by nothing
   but the callee's parameter, and the resolution runs only when the callee
   only reads that parameter's members (ADR 0320's closed-callee census). No
   other code can reach it, so the member-name census
   (`own_member_may_be_replaced`) is not consulted for it.

## Proof obligations

| Claim | Premise | Refused by | Cases |
| --- | --- | --- | --- |
| "This property's value is this function" | exactly the peeled property value, static key, last own property, no later spread, no getter | computed key, `__proto__`, spread after, getter | `SpreadAfter` |
| "The function runs with no other receiver use" | arrow, or a `function` expression the literal holds none of whose functions use `this`/`super` | receiver-using `function` in the literal | `ReceiverFunction` |
| "Only the callee holds the literal" | the literal is the argument itself; the callee only reads members of the parameter | a callee handing the parameter on; a literal behind a conditional; a bound literal | `HandedOn`, `Alternatives`, `BoundLiteral` |
| "Its reads are not lost" | the literal is not a summary node; reads stay with the function that writes it | n/a | `ReadingArrow` |
| "Identity is unique" | file and span | n/a | |
| "A cached answer is current" | the literal is in the caller's file; the callee's census is a consulted `FunctionSource` | n/a | `literal_property_dispatch_edits_match_cold` |

Self-review against the bug families found so far:

- **Key/receiver/origin identity.** The key is `property_static_key`
  (quoted keys included, computed and `__proto__` refused); receiver use is
  refused for the whole literal; origin is the literal written at the call.
- **Visible references only.** The literal has no name; the only reference is
  the callee's parameter, whose every use the closed-callee census reads.
- **Script files.** A callee in a script file is still read by its own
  census; nothing here relies on module scope.
- **Span equality.** The identity and the property value compare peeled
  spans; a method's nested function expressions carry the method's name and
  are excluded.
- **Nested function confusion.** Only the property's own value is admitted,
  never a function nested inside it.
- **`__proto__`/`super`.** A `__proto__:` key is not static; `super` occurs
  only in methods, which keep their method symbol.
- **Skippable awaits, primitive-by-cast, destructured parameters.** Not read
  here; a destructured callee parameter is not a closed callee, so it stays
  unresolved.
- **Merged alternatives.** A conditional over two literals is not the
  argument itself: the name census applies and the identity is not given.
- **Forwarding.** A callee forwarding the parameter whole to an exact callee
  that only reads its members keeps it closed (ADR 0320's forwarding census).
- **Retained staleness.** The twin edits the caller (spread after) and the
  callee in another file (hand-off), both directions.

## Consequences

49-project corpus against `/tmp/sweep-final13.json`: **76 `SC9012`
uncertifiable removed, 17 added** (all uncertifiable: 7 diverging and 1
unresolved member dispatches the next member of the same literal now raises,
3 forwarded uses, 6 `SC1001` "read through" claims where a property holding a
named function now resolves), 0 violations changed: 4,110 to 4,051. Fixtures:
`literal-property-dispatch` (new); `forwarded-tracked-compute`,
`invocation-flow`, `member-value-copiers`, `project-callback-uses`,
`provider-array-provenance` and `use-site-role` lose only `SC9012` dispatch
obligations whose arrows' reads keep their findings. Residual: a literal
behind a conditional or bound to a name, a callee that hands its parameter
on, and `function` expressions using their receiver stay unresolved.
