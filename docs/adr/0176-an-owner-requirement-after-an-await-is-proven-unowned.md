# ADR 0176: An owner requirement after an `await` is proven unowned

- Status: accepted and implemented (2026-10-04). The owner chose app-side
  owner proof as the next misuse lever on 2026-10-04.
- Owners: the owner analysis (`owners::AwaitContinuations`, both owner
  passes), the dialect's fresh-stack answer
  (`Dialect::fresh_stack_callback_owner`, unchanged in value), and the
  producer's `calls_after_await` dominance fact (unchanged).
- Relation: extends the fresh-stack scheduler edges
  (`fixtures/reactive-ir/fresh-stack-scheduler-owner`) from callbacks a host
  queue runs to the continuation an `await` resumes. No new wire field.

## Context

Local certification of the 38 app-import apps (2026-10-04) left 88 new
`missing-owner` sites uncertifiable. Read one by one, nearly all are correct
code: hooks such as `useMutation`, `createRAF` or `createEventListener` called
in a component body or in a helper a component calls, where the checker cannot
see the caller. The exception is
`readingroom/frontend/src/routes/settings/integrations.tsx`, which calls
`createTimer` after `await writeClipboard(…)`. That call has no owner whoever
called the function, because code after an `await` resumes from a promise
continuation.

The owner analysis did not know that. An owner-requiring call after an `await`
inherited the enclosing function's context. So an async helper called from a
component body was treated as owned and reported nothing, and an exported one
was a caller obligation (uncertifiable).

## Decision

1. **The continuation is a fresh stack.** A call that the producer's
   `calls_after_await` lists runs after an `await` on every path through its
   async function's own body. It is the fact `reactive-read-after-await`
   already proves reads with: branches, `&&`/`||`/`??`, `try`/`catch`, loops
   and `switch` merge conservatively, and nested closures are not scanned.
   Its owner is the dialect's fresh-stack answer. Only `CallbackOwner::None`
   applies the rule, and Solid 2 answers `None`.
2. **The operation is proven unowned.** For an effect, a cleanup or a
   contract call with an owner requirement, the context becomes
   unowned-and-proven. The owned, leaf, component-uncertain and later-run
   bits are cleared. The contract's own `guaranteed` flag still decides
   whether a contract call is a violation or stays uncertifiable.
3. **`onSettled`'s returned cleanup is out of scope.** It keeps the enclosing
   context, because whether an unowned settle registers is a separate
   question. JSX boundaries are not calls and are unaffected.
4. **The finding says why.** Its evidence reads "this call runs after an
   `await`, from a promise continuation: the owner current before the `await`
   is not current after it, and no other owner is".

## Consequences

- A captured owner restored with `runWithOwner(owner, () => …)` after the
  `await` is unaffected: the callback is a nested closure. It keeps its
  nullable-owner answer.
- A `createRoot` callback that is itself async keeps its lexical root answer
  after the `await`. That is a missed report, never a false one.
- `readingroom`'s `createTimer` stays uncertifiable. Its contract publishes
  the cleanup with `min: 0`, because `createTimer` registers an `onCleanup` for
  a number delay and an effect for an accessor. That is the cleanup/effect
  disjunction ADR 0173 leaves open.

## Evidence

- Runtime, published `@solidjs/signals` 2.0.0-rc.0, rc.3, rc.6, rc.8 and
  rc.9 dev builds and the rc.9 prod build, under Node 24, inside a
  `createRoot`:
  - `getOwner()` is non-null before the `await` and `null` after it;
  - an `onCleanup` after it raises `NO_OWNER_CLEANUP` (dev) and does not run
    when the root is disposed, while one before it does;
  - a `createEffect` after it raises `NO_OWNER_EFFECT` (dev).
- Fixture `fixtures/reactive-ir/owner-after-await`:
  - four violations that HEAD reported as nothing (three) or uncertifiable
    (one);
  - three negatives stay clean;
  - the captured-owner case stays uncertifiable.

  `App.tsx` type-checks against the real `solid-js@2.0.0-rc.9` typings with
  no diagnostic.
- Coverage: 160 fixtures, 829 findings, and no other fixture moved.
- Sweep over the 38 apps (48 projects), host-free and browser:
  - 264 -> 264 violations, 0 added, 0 removed;
  - the uncertifiable counts are unchanged, and so are the messages.

  The one after-`await` cleanup in the corpus
  (`app-game/.../rotating-cube-2.tsx:303`) was already a violation through
  its `onSettled` caller.

The rule closes a class of missed defects. It adds no finding on this corpus.
