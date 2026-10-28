# local-alias-bindings

**Claim (ADR 0463).** A `const` that holds exactly another local `const` variable, `const useIt = r1` or a slot of a destructured object literal `const { useIt } = { useIt: r1 }` (keyed, string-keyed, shorthand, chained), is that variable for every rule: reads, writes, store mutations and action calls through it are reported where the direct form is.

Violations: `strict-read-untracked` in a component body (directly and through a named closure it calls), a `createRoot` body and an effect apply (signal and memo); `pending-async-unsuspendable-read` (async memo, both forms); `reactive-read-after-await`; `no-write-in-pure-computation` (signal and store setter slots); `no-store-mutation-outside-setter` (store alias); `action-called-in-owned-scope` (action slot).

Uncertifiable: a mention that may run before the alias's declarator (`[1].forEach(() => useLate())` above `const useLate = r1`) throws `ReferenceError`, so its read and write are unplaced (ADR 0339's proof decides each mention).

Silent: the alias read in a memo, in JSX and under `untrack`; an alias of a plain function; after an `await`, a source or store key the computation read before it through the source or the alias.

Refused, no read: a `let` alias, a shadowing name, a parameter alias, a literal with a spread, a computed key or a getter, and a default. Where the type says `Accessor`, the call stays the SC9012 obligation it was (ADR 0294); so does a conditional alias. An exported alias and a `let` or `var` source are not recorded (`solid-facts` unit test).

tsc: clean against the stub (copied from `write-render-apply-row-layers`) and against the published rc.17 typings. Runtime: each component alone on the fuzzer's rc.17 fast path; every violation reports its code, `BeforeDeclarator` throws `ReferenceError`, the silent components report no read or write code (an async memo rendered with no `Loading` reports `ASYNC_OUTSIDE_LOADING_BOUNDARY`, uncertifiable here); the refused ones warn `STRICT_READ_UNTRACKED` (known misses).
