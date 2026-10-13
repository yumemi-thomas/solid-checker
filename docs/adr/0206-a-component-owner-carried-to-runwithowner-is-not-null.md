# ADR 0206: A component's owner carried to runWithOwner is not null

- Status: accepted and implemented (2026-10-06). Track A, step 5 of the
  2026-10-06 plan.
- Owner: `component_owner_binding`, called from
  `run_with_owner_callback_owner` in `solid-reactive-ir/src/owners.rs`.

## Context

`runWithOwner(owner, fn)` takes `Owner | null`, and `getOwner()` returns
`Owner | null`. So the checker answered `Conditional` for every owner value
it could not see. Everything `fn` reaches that needs an owner then stayed an
uncertifiable `missing-owner` result:

> onCleanup is called without a reactive owner … runWithOwner may receive
> null, so solid-checker cannot prove this execution has an owner

The idiom that carries a component's owner across an `await` is exactly
this, and it is what `sefer`'s `ProjectProvider` does:

```tsx
export function ProjectProvider(props) {
  const owner = getOwner();
  void composeServices(…).then((services) => {
    runWithOwner(owner, () => { onCleanup(…); return makeShell(…); });
  });
  …
}
```

Those are all 14 uncertifiable `missing-owner` results on the rc.13 corpus.

## Decision

The owner argument of `runWithOwner` is proven non-null (`Creates`, which
also covers a supplied owner proven non-null) when it names a binding that
meets all of these:

- it is a `const` identifier binding;
- it is initialized by a call the dialect resolves to `getOwner`;
- that call is written directly in a proven component's body, not in a
  nested function or a default parameter. The component is not async and
  not a generator.

A proven component's body runs under an owner: the owner graph seeds it
owned, as its render does. `getOwner()` there returns that owner.

## Consequences

- The owner may have been disposed by the time the callback runs. That is
  not the null owner this rule is about. A cleanup registered on a disposed
  owner is a separate question, and the checker makes no claim about it.
- Still conditional:
  - `getOwner()` in a helper that is not a proven component (even one only
    called from a component body), or in a nested function;
  - a `let` binding;
  - an owner passed through a parameter or a property.

## Evidence

- **Fixture** `fixtures/reactive-ir/component-owner-binding`:
  - `Carried` is clean;
  - the helper, `let` and nested-`getOwner` cases stay `SC4001`
    uncertifiable.
- **Coverage:** 176 fixture projects, 927 findings. Only the new snapshot is
  new.
- **rc.13 corpus**, browser host, release binary, against
  `rc13-g-browser.json`:
  - uncertifiable 3,435 to 3,421, with exactly the 14 `sefer` sites removed
    and none added;
  - violations unchanged at 285.
