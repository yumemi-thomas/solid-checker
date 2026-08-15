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
and the expression dominates every later read in the same function body. Both
proofs exclude nested closures — a callback created after the await has its own
tracking story. Props member reads follow the component's caller classification
(see [strict-read-untracked](strict-read-untracked.md)): proven-static props are
not reactive and stay silent; unprovable ones are reported as **uncertifiable**.

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
