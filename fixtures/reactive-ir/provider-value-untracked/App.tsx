import { createContext, createMemo, createSignal, untrack, useContext } from "solid-js";
import { Count } from "./Contexts";

const Theme = createContext<number>();
const [count] = createSignal(1);

// Positive: the provider reads `value` once; the child never sees a write.
export function Provided(props: { children?: unknown }) {
  return <Theme value={count()}>{props.children}</Theme>;
}

// Positive: a memo accessor is read there just the same.
export function Derived(props: { children?: unknown }) {
  const doubled = createMemo(() => count() * 2);
  return <Theme value={doubled()}>{props.children}</Theme>;
}

// Positive: the context is made in another file; the element still resolves to
// the exact `createContext` binding.
export function Imported(props: { children?: unknown }) {
  return <Count value={count()}>{props.children}</Count>;
}

// Negative: a function value is stored, and its body runs when someone calls it.
export function Lazy(props: { children?: unknown }) {
  const read = () => count();
  return <Theme value={read as unknown as number}>{props.children}</Theme>;
}

// Negative: an explicit one-time snapshot.
export function Snapshot(props: { children?: unknown }) {
  return <Theme value={untrack(() => count())}>{props.children}</Theme>;
}

// Negative: a constant reads nothing.
export function Constant(props: { children?: unknown }) {
  return <Theme value={1}>{props.children}</Theme>;
}

// Negative: the child's read of the context value is its own, tracked in JSX.
export function Child() {
  const value = useContext(Theme);
  return <span>{value}</span>;
}
