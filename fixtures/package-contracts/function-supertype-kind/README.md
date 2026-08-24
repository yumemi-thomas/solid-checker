# The `Function`-supertype family, pinned wrong on purpose

This fixture pins a **known-wrong** `kind` claim, so that the producer
follow-up which fixes it is visible as a flip in a gate rather than as a
paragraph nobody reads.

`export_kind_proof` decides an export's runtime `kind` from two facts:
`Callability` and `Constructability`, both demanded at the export-specifier
span. Either positive proves a runtime function; the two closed negatives
together prove a value. That is full proof for every type **outside**
lib.es5.d.ts's signature-less `Function`-supertype family.

Inside it, the pair is wrong. `Function` declares `apply`/`call`/`bind` and no
call or construct signature, so `nonCallable` + `nonConstructable` is a truthful
answer *about the declared type* — while every function value in JavaScript is
assignable to `Function`, so `typeof x === "function"` can hold at runtime for
every value the type admits. `object`, `{}` and `Record<string, unknown>` are
the same shape reached without naming `Function` at all. TypeScript-Go's own
`typeof` narrowing (`checker.isFunctionObjectType`) gets these right through a
`bind`-member subtype-of-`Function` fallback that the producer's
`callabilityOfType` / `constructabilityOfType` walks do not carry, so the
compiler's own answer and the fact pair's answer diverge **by design** for this
family. See the producer's ADR 0020, "What it does not answer".

Nothing on the consumer side can detect it: assignability to `Function` is not
one of these facts, and there is no honest local substitute — matching the
rendered type text for `Function` is exactly the `typeDescriptor.text`
interpretation the producer's migration notes forbid, and an alias
(`type Handler = Function`) defeats it anyway.

Expected generation, all five exports on one summary:

| Export | Declared type | Published `kind` | Correct? |
| --- | --- | --- | --- |
| `raw` | `Function` | `value` | **no** — may be a function at runtime |
| `bag` | `object` | `value` | **no** — same family |
| `empty` | `{}` | `value` | **no** — same family |
| `table` | `Record<string, unknown>` | `value` | **no** — same family |
| `retries` | `number` | `value` | yes — the control |

`retries` is the reason the pair cannot simply be distrusted wholesale: it
answers the identical closed negative and `value` is the right claim for it.

## Not a regression of the constructability fact

This hole predates the constructability wiring and was not widened by it.
Through `callability` alone every one of these answered `nonCallable`, no class
syntax contradicted it, and the generator published `value` for exactly the same
reason. The change to the rule moved none of these rows.

## Why the stubs are the real types

Every type here comes from the default library, and this fixture declares no
types of its own. That is load-bearing: a stub that redeclared `Function` with a
call signature, or loosened `Record`, would manufacture a different outcome and
this pin would stop describing anything real.

## The flip that fixes it

The producer follow-up named in ADR 0020 — give `constructabilityOfType` and
`callabilityOfType` the same `bind`-member subtype-of-`Function` fallback
`isFunctionObjectType` already has. When it lands, `raw`, `bag`, `empty` and
`table` answer `Unknown` on both facts instead of a closed negative,
`export_kind_proof` returns `Unresolvable`, and `promote_entry_callable`
**refuses this entrypoint** rather than publishing it — while `retries` keeps
`value`. Because `.` is the only entrypoint here, the refusal costs the whole
contract, so the flip in `expected.json` is a generation failure, not a
changed summary. Split `retries` onto its own entrypoint at that point.

Tracked in docs/precision-backlog.md.
