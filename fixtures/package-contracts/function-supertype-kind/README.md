# Signature-less `Function` values are still functions

This fixture pins the schema-15 `Callability::UntypedCallable` migration.
`export_kind_proof` decides an export's runtime `kind` from `Callability` and
`Constructability`: either positive proves a function, while the two closed
negatives together prove a value.

`Function`, `CallableFunction`, `NewableFunction`, aliases and interfaces based
on `Function`, and intersections containing `Function` declare no readable call
signature. They nevertheless admit only JavaScript function objects. Type Facts
therefore reports the distinct positive `UntypedCallable` answer: enough to
publish `kind: "function"`, but not enough to infer parameters, arity, callback
execution, or a call signature. The raised summaries consequently keep
`callbacks: { "status": "unknown" }`.

The boundary is narrower than the old fixture claimed. `object`, `{}`, and
`Record<string, unknown>` may hold functions, but they also admit non-function
values. At those declared types the closed negative pair is honest, so they stay
`kind: "value"` beside the `number` control.

Expected generation:

| Export | Declared type | Published `kind` |
| --- | --- | --- |
| `raw` | `Function` | `function` |
| `callable` | `CallableFunction` | `function` |
| `newable` | `NewableFunction` | `function` |
| `alias` | alias of `Function` | `function` |
| `extended` | interface extending `Function` | `function` |
| `branded` | intersection containing `Function` | `function` |
| `bag` | `object` | `value` |
| `empty` | `{}` | `value` |
| `table` | `Record<string, unknown>` | `value` |
| `retries` | `number` | `value` |

Every type comes from the real default library; the fixture does not redeclare
`Function` or loosen a signature. Producer ADR 0021 and
docs/precision-backlog.md record the residual limits: `UntypedCallable` does
not supply signature facts, constructability remains independent, and union
constituent precision is unchanged.
