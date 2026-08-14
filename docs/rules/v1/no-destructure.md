# v1/no-destructure

`SC1003` · **error** · violation · 🛠️ safe fix available

Component props or another proven reactive object are destructured outside a
tracking scope, which unwraps each property once and severs reactivity.

## What it does

Flags object destructuring of a component's props — both in the parameter list
(`function Card({ title })`) and in later bindings (`const { title } = props`) —
and of objects proven reactive by native APIs or reviewed package contracts.
It does not guess from hook names such as `useParams`.

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

const { id } = useParams(); // Flagged when the package contract says this is a store.
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

// Destructuring inside a tracking computation reads the latest values.
const selection = createMemo(() => {
  const { id } = useParams();
  return id;
});
```

## How to fix

Keep a reactive object intact and read `object.<name>` inside JSX or a tracked
computation. To split props use `splitProps(props, [...keys])`; to default them use
`mergeProps(defaults, props)`. Parameter destructuring is always setup-time;
destructuring inside a tracked memo or effect is safe.

`splitProps` returns a tuple of prop proxies, so binding it with array
destructuring is safe: it is the *property* access that must stay deferred.

## Related

- [v1/strict-read-untracked](./strict-read-untracked.md) — the general untracked-read rule
