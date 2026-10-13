# ADR 0275: A helper's operation counts only on every completion

- Status: accepted and implemented (2026-10-13).
- Owners:
  - `solid-reactive-ir/src/cleanup.rs`: `Entry`, `reached_on_every_call` and
    the `Reached` operation list of the leaf-scope walk;
  - `solid-facts` `ast::synchronously_unconditional_calls`;
  - the via wording in `rules/src/lib.rs`.
- Narrows the statically-present-code premise (2026-10-13) at a call
  boundary. No fact schema, contract or receipt change.

## Context

The leaf-scope walk (ADRs 0179, 0204, 0209, 0210) credits a call in a leaf
owner's callback with every forbidden operation written in the called
helper's synchronous extent. A helper's branch can depend on the arguments of
the call that reaches it, so "the helper contains it" does not mean "this
call performs it". `@solid-primitives/timer`'s `createTimer` registers
`onCleanup` for a numeric delay and `createEffect` otherwise; called with
`30_000` inside `onSettled`, the walk reported both as violations
(rc.13 corpus experiment of 2026-10-13, with the package's shipped source
vendored into readingroom). The same shape written as a local helper gave the
same false `createEffect` violation.

## Decision

1. **Only an operation on every normal completion of a called body is
   performed by the call.** The walk enters a body either as the leaf
   callback itself or as a call from the leaf scope. In a called body, an
   operation, or a call to the next helper, counts as performed only when the
   body runs it on every completion its caller can continue from. Every hop
   above it must be unconditional in the same sense. As amended after review
   round 1 (`../solid-checker-rc13-corpus/experiments/review-0276/REVIEW-1.md`),
   an earlier `throw` counts as a way out: in `try { helper(false) } catch {}`
   the caller carries on with the operation skipped. ADR 0161's
   `unconditional_calls` keeps admitting a throw, because its claim is about
   normal completions.
2. **Anything else is possible, not absent.** It is projected as an
   uncertifiable `leaf-owner-forbidden-call` whose wording says the helper
   performs it on some calls, never as silence.
3. **The leaf callback's own body is unchanged.** A branch written there stays
   under the statically-present-code premise, because the code at the site is
   the defect whatever the branch decides.
4. **An `async` helper states its calls before its first suspension.** Its
   synchronous prefix runs on the call. Calls that an `await`, `for await` or
   `await using` of its own precedes or contains are not credited. A
   generator's body does not run on the call.
5. **ADR 0210 is unchanged.** An inline standard-library callback inherits the
   standing of the call that hands it over, although it may run any number of
   times.

## Consequences

- rc.13 corpus (browser host): 573 → 573 violations, 3,719 → 3,719
  uncertifiable. No current corpus finding depended on a conditional helper
  operation.
- In the vendored timer experiment, the six readingroom findings at the three
  `createTimer` sites are now uncertifiable, including the false
  `createEffect` ones. The real `onCleanup` misuse is an obligation natively,
  because nothing here knows the delay is a number. The authored contract
  still proves it.
- Not yet: argument sensitivity, which would decide `typeof delay ===
  "number"` from the call. A helper first visited on a conditional path keeps
  its operations possible if a later hop reaches it unconditionally.

## Evidence

- Fixture `reactive-ir/leaf-scope-helper-completion`: violations for an
  unconditional helper, two unconditional hops, and a branch in the leaf
  callback. Uncertifiable for the timer shape (twice), a guarded
  registration, a guard that throws, a loop, a conditional second hop, a
  conditional `await` and a logical assignment (`||=`). Clean for a helper with no forbidden operation. `tsc --noEmit`
  passes on it.
- `fp-owner-leaf-helper-after-await` keeps its before-await violation
  (rule 4).
- Unit test
  `an_async_function_states_its_calls_before_its_first_suspension`.
