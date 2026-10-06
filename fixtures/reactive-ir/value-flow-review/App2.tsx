import { createSignal } from "solid-js";
import { evens, external } from "./helpers";

// SC9012: a computed read names its key by value.
function Computed(props: { items: number[] }) {
  const picked = evens(props["items"]);
  return <div>{picked.length}</div>;
}

// SC9012: `var` redeclares the parameter with another value.
function Redeclared() {
  function forward(list: number[]) {
    var list: number[] = external();
    return evens(list);
  }
  const picked = forward([1]);
  return <div>{picked.length}</div>;
}

// SC9012: a defaulted previous value is not the previous value.
function DefaultedPrevious() {
  const [items, setItems] = createSignal<number[] | undefined>(undefined);
  const reset = () => setItems((previous = external()) => previous);
  const picked = evens(items() ?? []);
  return <div onClick={reset}>{picked.length}</div>;
}

type Replaceable = { items: number[]; replace(this: Replaceable): void };

// SC9012: `props.replace()` hands `props` to user code as `this`.
function Replaced(props: Replaceable) {
  props.replace();
  const picked = evens(props.items);
  return <div>{picked.length}</div>;
}

export function App2() {
  return (
    <div>
      <Computed items={[1]} />
      <Redeclared />
      <DefaultedPrevious />
      <Replaced
        items={[1]}
        replace={function (this: Replaceable) {
          this.items = external();
        }}
      />
    </div>
  );
}
