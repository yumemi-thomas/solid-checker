# v1/no-destructure

`SC1003` · **error** · violation · 🛠️ safe fix available

Component props are destructured, which unwraps each property once and severs
reactivity.

## What it does

Flags object destructuring of a component's props — both in the parameter list
(`function Card({ title })`) and in later bindings (`const { title } = props`).

When every destructured property is only read (never reassigned), solid-checker
offers a safe fix that restores the `props` parameter and rewrites the body to
`props.<name>` accesses.

## Why is this bad?

In Solid, `props` is a reactive object: the *property access* is what subscribes.
Destructuring performs every access once, at component setup, and binds the plain
values. The component renders correctly the first time and then never updates when
the parent passes new props — one of the most common sources of "my UI doesn't
update" bugs.

## Examples

Examples of **incorrect** code for this rule:

```tsx
function Card({ title, body }) {
  return (
    <article>
      <h2>{title}</h2>
      <p>{body}</p>
    </article>
  );
}

function Avatar(props) {
  const { src } = props; // Same problem, one statement later.
  return <img src={src} />;
}
```

Examples of **correct** code for this rule:

```tsx
function Card(props) {
  return (
    <article>
      <h2>{props.title}</h2>
      <p>{props.body}</p>
    </article>
  );
}

// Splitting and defaulting props without destructuring:
function Field(props) {
  const merged = mergeProps({ type: "text" }, props);
  const [local, rest] = splitProps(merged, ["label"]);
  return <input {...rest} aria-label={local.label} />;
}
```

## How to fix

Keep the `props` object intact and read `props.<name>` inside JSX or a tracked
computation. To split props use `splitProps(props, [...keys])`; to default them use
`mergeProps(defaults, props)`. Never destructure — not in the parameter list, not in
the body, and not in control-flow callbacks.

`splitProps` returns a tuple of prop proxies, so binding it with array
destructuring is safe: it is the *property* access that must stay deferred.

## Related

- [v1/strict-read-untracked](./strict-read-untracked.md) — the general untracked-read rule
