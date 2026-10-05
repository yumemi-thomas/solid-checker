# reactive-read-after-await

`SC1002` · **error** · violation

A reactive accessor is read after an `await` inside an async computation, where
dependency tracking has already ended.

## What it does

Flags reads of signal accessors, store paths, and props that occur after the first
`await` in an async function passed to a computation (`createMemo`, `createEffect`,
`createProjection`, and friends).

Accessor **calls** are proven by TypeScript-side dominance analysis, which handles
branches, loops, `switch`, and `try`/`finally` precisely (both-branch awaits
dominate; a conditional or looped await does not). Store-path and props **member
reads** are proven against the function's straight-line awaits: an await with no
conditional, logical, loop, switch, or try construct between the function entry
and the expression dominates every later read in the same function body. Props
member reads follow the component's caller classification (see
[strict-read-untracked](strict-read-untracked.md)): proven-static props are not
reactive and stay silent; unprovable ones are reported as **uncertifiable**.

Both proofs exclude nested closures by default, with one proven exception. A
function written **directly** in the argument of the exact built-in
`Array`/`ReadonlyArray.prototype.filter` runs inline, before the awaiting
computation resumes, so both proofs continue into that callback's body with the
callback as the owning function. The exception is deliberately narrow, and each
of these keeps it from applying:

- a `.filter` that does not resolve to the built-in declaration — a
  project-defined or shadowed method, or an unresolved/package callee;
- an argument that is not the literal function — `filter(makePredicate(fn))`
  hands the callback to a wrapper that may run it later;
- an `async` callback, which suspends at its own first await;
- a deferred standard callback such as `Promise#then`;
- an awaiting function with no *straight-line* await, since the member-read
  site the extension hangs off requires one — a `try`-wrapped await disables
  it even though accessor-call dominance alone would have proven the read.

Nothing else in the standard library is treated as synchronous yet, and the
extension reports only inside the awaiting function's own directly written
filter callbacks (it does not recurse into a filter nested in another one).

**A source already tracked is exempt** (ADR 0195). A read after the await of
an accessor that the same function already called *on every run, before its
first suspension* is not reported: that earlier call ran inside the tracking
window, so the source is a dependency and the computation re-runs when it
changes. The earlier call must be in the function's own straight-line flow
(not under a branch, logical operator, loop, switch, `try` or optional chain,
and not in a nested function) and must precede every suspension, a
conditional `await` included. Only accessor calls are exempted this way;
store-path and props member reads are not.

**The runtime agrees from rc.13.** `@solidjs/signals@2.0.0-rc.13` warns
`UNTRACKED_READ_AFTER_AWAIT` in dev for a read after the first `await` of an
async computation. It is silent inside `untrack`, before the first `await`,
for a source already a dependency, in a plain async function, in an async
effect apply or tracked effect, and in an action body; the rule is silent in
each of those cases too (the rc.13 review, runtime § 3.4.2). The warning
needs V8 async stack frames and is absent from production builds.

## Why is this bad?

Tracking is synchronous: a computation collects dependencies only until its first
`await`. A read after that point registers no dependency, so the computation never
re-runs when the value changes — the async result is permanently stale with respect
to that input.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const profile = createMemo(async () => {
  const posts = await fetchPosts();
  // Tracking ended at the await: changing userId() never re-runs this memo.
  return posts.filter((post) => post.author === userId());
});
```

Examples of **correct** code for this rule:

```tsx
const profile = createMemo(async () => {
  // Read every reactive input before the first await…
  const id = userId();
  const posts = await fetchPosts();
  // …and use the captured value afterwards.
  return posts.filter((post) => post.author === id);
});

// Or split the post-await dependency into its own synchronous computation:
const posts = createMemo(() => fetchPosts());
const profile = createMemo(() => posts().filter((post) => post.author === userId()));
```

## How to fix

Read reactive values before the first `await` and carry the results through the
async work. If a value must stay live after the `await`, split the read into its
own synchronous computation and compose the two.

## Related

- [strict-read-untracked](strict-read-untracked.md) — the synchronous variant
- [async-outside-loading-boundary](async-outside-loading-boundary.md) — consuming async computations
