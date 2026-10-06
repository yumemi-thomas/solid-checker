import { createMemo, createSignal } from "solid-js";
import { evens, external, joined } from "./helpers";

// SC9012: the array's own `join` is assigned.
function Rejoined() {
  const list = ["a"];
  list.join = () => "b";
  const text = joined(list);
  return <div>{text}</div>;
}

// SC9012: the updater writes its previous value before returning it.
function OverwrittenPrevious() {
  const [items, setItems] = createSignal<number[]>([]);
  const reset = () =>
    setItems((previous) => {
      previous = external();
      return previous;
    });
  const picked = evens(items());
  return <div onClick={reset}>{picked.length}</div>;
}

// SC9012: a destructuring write rebinds the parameter.
function Destructured() {
  const forward = (list: number[]) => {
    [list] = [external()];
    return evens(list);
  };
  return <div>{forward([1]).length}</div>;
}

// SC9012: a local `Array` is not the global.
function LocalArray() {
  class Array extends globalThis.Array<number> {}
  const picked = evens(Array.from([1]));
  return <div>{picked.length}</div>;
}

// SC9012: `split` defers to its separator's `Symbol.split`.
function ProtocolSeparator() {
  const separator = { [Symbol.split]: (_input: string): string[] => ["x"] };
  const text = joined("a,b".split(separator));
  return <div>{text}</div>;
}

// SC9012: a memo's options can supply a value no compute returns.
function MemoOptions() {
  const items = createMemo(() => [1], { equals: false });
  const picked = evens(items());
  return <div>{picked.length}</div>;
}

// SC9012: `props` is aliased, so a tag does not show what it holds.
function Aliased(props: { items: number[] }) {
  const alias = props;
  const picked = evens(alias.items).concat(evens(props.items));
  return <div>{picked.length}</div>;
}

export function App() {
  return (
    <div>
      <Rejoined />
      <OverwrittenPrevious />
      <Destructured />
      <LocalArray />
      <ProtocolSeparator />
      <MemoOptions />
      <Aliased items={[1]} />
    </div>
  );
}
