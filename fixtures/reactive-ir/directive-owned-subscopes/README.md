# directive-owned-subscopes

`primitive-in-directive-application` (SC6001) is reported only for code that runs in an intrinsic `ref`'s apply phase with no owner: inside a function that is itself the ref value (alone or as an array element) and inside no call of the ref expression. Runtime-probed on rc.13 (an `onCleanup` or `createEffect` witness raises no `NO_OWNER_*` where this stays silent):

- an eager argument (`ref={tip(createMemo(...))}`) or an IIFE that builds the callback runs while the component renders: silent;
- a creation in a `createRoot` callback inside the apply phase, directly or through synchronous nesting (an IIFE, an inline `untrack`), is owned by that root: silent. Under `runWithOwner(owner, …)` the owner may be `null`: uncertifiable;
- a function nested in such a callback that a host scheduler runs (a timer) has no owner: a violation; one handed to a project helper (`own(() => …)`) may run under an owner of the helper's: uncertifiable;
- a default parameter initializer runs in its function's call: in the apply callback a violation, in a root's callback owned;
- a callback a call in the expression returns (`ref={(() => (el) => …)()}`), a factory's result anywhere but as the ref value or a ref-array element, a function nested in the applied callback that nothing visibly invokes, and a generator (calling one runs none of its body) are undecided: uncertifiable.

Positives: a memo directly in apply, each ref-array callback, the timer case. Explicit roots are uncertifiable (ADR 0355): disposal or an empty body may prevent any leak. RootLifetime covers namespace, factory and shadowed variants. Signatures copy signals rc.13 `dist/types`.
