# no-destructure

`SC1003` · **error** · violation · 🛠️ safe fix available

Component props or another proven reactive object are destructured outside a
tracking scope, which unwraps each property once and severs reactivity.

## What it does

Flags object destructuring of a component's props — both in the parameter list
(`function Card({ title })`) and in later bindings (`const { title } = props`) —
and of objects proven reactive by native APIs or analyzed dependency source.
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

A store's destructure claims the client's strict-read window, as `strict-read-untracked` does, so it is silent where no client
build runs it: in a component or helper only an `isServer` arm reaches (ADR 1158); under an undecided build constant it is
uncertifiable. A props destructure is a pattern claim of any build and stays.

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

const { id } = useParams(); // Flagged when analyzed source proves this is a store.
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

eslint-plugin-solid's `reactivity` rule also reports destructured props
(`untrackedReactive`); here that claim is this rule's alone, so disabling the
`reactivity` key ([ADR 0428](../decisions.md#adr-0428)) does not turn it off.

## Related

- [strict-read-untracked](strict-read-untracked.md) — the general untracked-read rule

## Uncertifiable

A destructure no exact root reaches, blocker `destructure-root-unproven` (ADR 0657): an
assignment, a `for…of`, a wrapped initializer such as `merge(…, props)`, a default, a rest
pattern, and a hook's result, unless every return of a project hook is a closed object
literal. A read of one member (`createSignal(props.a)`) is `strict-read-untracked`'s, not
this rule's. A first parameter is a props pattern whatever follows it.

A violation stands only where setup provably runs the destructure: at module scope or in a component's body, through layers that run in place there. A destructure in a helper the body calls, an effect's apply, an `untrack` callback, a ref's application, a function nested in a named callback, a listener or an object's method is uncertifiable ([ADR 1106](../decisions.md#adr-1106)).

ADR 1109: a destructure in a callback a carrier provably never runs (a `const` holding `[]` that
only iterations read, a one-element `sort`) is uncertifiable (`function-unentered`). A store
destructured in a standard inline callback's parameter (`[store].map(({ v }) => v)`) is an SC9012
`value-in-carrier` row. A callback a helper calls on some path only (`if (c) f()`) stays a violation: the path counts as
reachable. One called after a statement that may throw, in a `try` whose `catch` completes
normally, is uncertifiable (the setup proof above does not hold).

A store destructure written anywhere else takes the verdict [strict-read-untracked](strict-read-untracked.md) gives each slot read as a member (`const { v } = store` is `store.v`): a violation where SC1001 proves one, uncertifiable where it leaves an obligation, no row where it proves a silence ([ADR 1107](../decisions.md#adr-1107)). The pattern is read by its properties' keys by value, never its text ([ADR 1122](../decisions.md#adr-1122)): a pattern with no property and no rest element reads nothing (comments aside); a sub-pattern that binds no name (`{ a: {} }`, `{ items: [] }`) reads its property. Each property is handed over as the Get of its key whenever the key is exact ([ADR 1162](../decisions.md#adr-1162)): a plain, renamed or quoted key with no escape (`{ v }`, `{ then: w }`, `{ "v": x }`), with or without a default, and a value that is a name or an empty nested pattern (`{ a: {} }`, `{ items: [] }`). So a `then` slot there is silent where its member twin `store.then` is (a literal `then` nothing writes). A computed key, a rest element, an escaped key or a non-empty nested pattern is not handed over (the setup verdict stands). Where it is not handed over, a computed key no literal names is uncertifiable (`store-key-undecided`), and so is a pattern whose every property reads `then` in any spelling: its Get does not warn. A quoted key or a literal computed key is a decided key.

A root may also be the sole element of the array literal a `for…of` iterates (`for (const { v } of [s])`), or the first source of `merge(s, {…})` whose later sources are closed literals without the pattern's keys (ADR 1193); any other root keeps `destructure-root-unproven`.
