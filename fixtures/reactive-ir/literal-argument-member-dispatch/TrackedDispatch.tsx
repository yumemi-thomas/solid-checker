import { untrack } from "solid-js";

function pageSlice<T>(items: readonly T[]): T[] {
  return items.slice(0, 2);
}

type Props = {
  items: readonly number[];
  keep: (read: () => number[]) => void;
};

// Existing direct-call control: the runtime implementation runs tracked.
export function DirectTracked(props: Props) {
  return <div>{pageSlice(props.items).length}</div>;
}

// Every reference to this exact helper invokes it from tracked JSX.
export function LocalTracked(props: Props) {
  const visible = () => pageSlice(props.items);
  return <div>{visible().length}</div>;
}

function Viewer(props: { value: number }) {
  return <div>{props.value}</div>;
}

// A compiler-proven property getter defers execution to its consumer.
export function ComponentProperty(props: Props) {
  const visible = () => pageSlice(props.items);
  return <Viewer value={visible().length} />;
}

// Follow two synchronous, unescaped local helper calls.
export function ChainedTracked(props: Props) {
  const inner = () => pageSlice(props.items);
  const visible = () => inner();
  return <div>{visible().length}</div>;
}

// One eager call keeps the dispatch obligation.
export function MixedCalls(props: Props) {
  const visible = () => pageSlice(props.items);
  const initial = visible();
  return <div>{initial.length + visible().length}</div>;
}

// A non-call reference makes the invocation census incomplete.
export function EscapedHelper(props: Props) {
  const visible = () => pageSlice(props.items);
  props.keep(visible);
  return <div>{visible().length}</div>;
}

// Tracking does not survive the await, even when JSX calls the helper.
export function AsyncHelper(props: Props) {
  const visible = async () => {
    await Promise.resolve();
    return pageSlice(props.items);
  };
  return <div>{String(visible())}</div>;
}

// A cycle supplies no closed proof of the helper's invocation contexts.
export function RecursiveHelper(props: Props) {
  const visible = (again: boolean): number[] =>
    again ? visible(false) : pageSlice(props.items);
  return <div>{visible(true).length}</div>;
}

// A tracked caller cannot override an inner tracking-clearing boundary.
export function ClearedTracking(props: Props) {
  const visible = () => untrack(() => pageSlice(props.items));
  return <div>{visible().length}</div>;
}

// The same spelling denotes two independent declarations and caller sets.
export function ShadowedHelper(props: Props) {
  const visible = () => pageSlice(props.items);
  const initial = visible();
  {
    const visible = () => pageSlice(props.items);
    return <div>{initial.length + visible().length}</div>;
  }
}

// Transparent TypeScript wrappers preserve the identifier call target.
export function WrappedCallee(props: Props) {
  const visible = () => pageSlice(props.items);
  return <div>{(visible as () => number[])().length}</div>;
}

// A mutable function binding is not covered by this proof.
export function MutableHelper(props: Props) {
  let visible = () => pageSlice(props.items);
  return <div>{visible().length}</div>;
}

// A call made through a retained closure is not a direct tracked invocation.
export function StoredCaller(props: Props) {
  const visible = () => pageSlice(props.items);
  props.keep(() => visible());
  return <div>{visible().length}</div>;
}

declare function opaque(read: (value?: number[]) => number[]): number;

// Defaults belong to a retained callback, not the property's getter call.
export function GetterDefaultCaller(props: Props) {
  const visible = () => pageSlice(props.items);
  return <Viewer value={opaque((value = visible()) => value)} />;
}

// Nor does constructing that callback under tracked JSX invoke its default.
export function JsxDefaultCaller(props: Props) {
  const visible = () => pageSlice(props.items);
  return <div>{opaque((value = visible()) => value)}</div>;
}
