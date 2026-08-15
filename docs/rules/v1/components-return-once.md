# v1/components-return-once

`SC1004` · **warning** · violation

A component's return value depends on a reactive condition, but a component body
runs exactly once.

> Severity deliberately differs from the Solid 2.0 catalog: this rule keeps
> upstream eslint-plugin-solid's advisory **warning** for existing 1.x
> codebases, while
> [`component-returns-conditionally`](../component-returns-conditionally.md)
> reports the same `SC1004` defect as an **error**.

## What it does

Flags components whose `return` statement is controlled by a condition that reads a
reactive value (a signal, store path, or prop) — early returns, `if`/`else` around
returns, `switch` statements whose cases return, logical expressions selecting
returned JSX (`return props.user && <Profile/>`), and conditional expressions that
select the returned JSX structure. Only the returned expression's own
conditional/logical spine counts; a ternary nested inside a JSX attribute of a
returned branch is a tracked binding, not a structural branch.

## Why is this bad?

Solid components are setup functions: the body executes once, and only the JSX
expressions inside the returned tree stay live. A reactive condition around the
`return` is evaluated a single time — whichever branch was taken at setup renders
forever, and the UI never switches when the condition changes.

## Examples

Examples of **incorrect** code for this rule:

```tsx
function Dashboard(props) {
  // Evaluated once: logging in after mount never swaps the branch.
  if (!props.user) {
    return <LoginPrompt />;
  }
  return <Overview user={props.user} />;
}
```

Examples of **correct** code for this rule:

```tsx
function Dashboard(props) {
  // One JSX tree; the condition lives inside it, where it stays tracked.
  return (
    <Show when={props.user} fallback={<LoginPrompt />}>
      {(user) => <Overview user={user()} />}
    </Show>
  );
}
```

## How to fix

Return a single JSX tree and move the branch into it: `<Show when={...}
fallback={...}>` for two-way branches, `<Switch>`/`<Match>` for multiple cases, or
a ternary inside JSX. Anything inside the returned tree re-evaluates reactively;
anything before the `return` does not.

## Related

- [v1/strict-read-untracked](./strict-read-untracked.md) — reads in the component body generally
