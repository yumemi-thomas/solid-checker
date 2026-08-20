# no-destructure

`SC1003` · **error** · violation · 🛠️ safe fix available

Component props or another proven reactive object are destructured outside a
tracking scope, which unwraps each property once and severs reactivity.

## What it does

Flags object destructuring of a component's props — both in the parameter list
(`function Card({ title })`) and in later bindings (`const { title } = props`) —
and of objects proven reactive by native APIs or reviewed package contracts.
It does not guess from hook names such as `useParams`.

Only setup-time destructures are reported. Destructuring inside a scope that runs
fresh at call time is legal at runtime and stays silent: event handlers,
`onSettled` and other deferred/leaf callbacks, `untrack` callbacks, an effect's
apply function, directive applications, tracked computations (which re-run and
re-subscribe), and body-defined handlers/helpers whose execution the engine cannot
pin to setup.

Props also follow their callers (probed against rc.0's `devComponent`): a
destructure that binds only props every call site passes statically unwraps plain
properties and misbehaves in no way — silent. Binding a prop some call site passes
reactively stays a **violation**. When the component's call sites cannot be
enumerated (exported, spread into, or referenced outside JSX), the finding is
reported as **uncertifiable** — a proof obligation, not a proven runtime defect.

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
  const rest = omit(props, "label");
  const merged = merge({ type: "text" }, rest);
  return <input {...merged} aria-label={props.label} />;
}

// Destructuring inside a tracking computation reads the latest values.
const selection = createMemo(() => {
  const { id } = useParams();
  return id;
});
```

## How to fix

Keep a reactive object intact and read `object.<name>` inside JSX or a tracked
computation. To split props use `omit(props, ...keys)`; to default them use
`merge(defaults, props)`. Parameter destructuring is always setup-time;
destructuring inside a tracked memo or effect is safe.

## Related

- [strict-read-untracked](strict-read-untracked.md) — the general untracked-read rule
