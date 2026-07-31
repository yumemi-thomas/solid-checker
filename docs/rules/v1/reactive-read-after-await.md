# v1/reactive-read-after-await

`SC1002` · **error** · violation

A reactive accessor is read after an `await` inside an async computation, where
dependency tracking has already ended.

## What it does

Flags reads of signal accessors, store paths, and props that occur after the first
`await` in an async function passed to a computation (`createMemo`, `createEffect`,
`createComputed`, and friends).

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

// Or let createResource own the async work: the source function is tracked,
// and the fetcher re-runs with its latest value whenever it changes.
const [profile] = createResource(userId, async (id) => {
  const posts = await fetchPosts();
  return posts.filter((post) => post.author === id);
});
```

## How to fix

Read reactive values before the first `await` and carry the results through the
async work. If a value must stay live across the `await`, move the async work into
`createResource`: reactive reads in its source function stay tracked, and the
fetcher receives the latest source value on every re-run.

## Related

- [v1/strict-read-untracked](./strict-read-untracked.md) — the synchronous variant
