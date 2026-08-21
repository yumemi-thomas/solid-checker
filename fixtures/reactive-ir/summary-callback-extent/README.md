# summary-callback-extent

A function's caller-visible read summary is about its **synchronous extent**.
Calling a helper performs only the reads that happen while the call runs, so a
read sealed inside a callback the primitive runs later — or runs in its own
tracking scope — is not the caller's read and must not be attributed to it.

The dialect's callback vocabulary decides this, and its three executions
divide cleanly. Only the first propagates:

| Execution | What the dialect says | Propagates to callers |
| --- | --- | --- |
| `Inline` | reads subscribe whatever was tracking at the call site | yes |
| `Tracked` | reads subscribe the callback's own observer | no |
| `Deferred` | reads subscribe nothing the caller owns | no |

The summary used to exclude exactly one shape — Solid 2.0's `createEffect`
*apply* slot, matched by primitive name and `Deferred`. A Solid 1.x effect's
callback is `Tracked`, so it fell through and leaked its read into the
enclosing helper's summary. Calling such a helper from a render scope then
produced a **proven SC1001 violation** for a read that never happens at the
call site, while the identical read inside the helper was correctly silent:
the intraprocedural and interprocedural halves of the analyzer disagreed, and
the interprocedural half was wrong. A false violation is worse than a missing
one, which is why this is pinned rather than left to the count.

`eagerArgument` is the case that keeps the rule from being "ignore every
callback slot". `onMount(compute(count()))` puts the read in a `Deferred`
slot, but there is no function literal between the read and the argument, so
`count()` is evaluated while the argument list is built and the read *is* the
caller's. Keyed on the slot alone it would be wrongly discarded;
`untrackedNow` covers the same requirement from the `Inline` side.

Every helper is local and `Host` is rendered at an exact JSX call site, which
proves its component identity under 1.x. So `Host` is a proven owner for the
helpers and a scope that does not track for the reads, and the owner question
stays out of this fixture: it reports exactly three findings, all violations.
