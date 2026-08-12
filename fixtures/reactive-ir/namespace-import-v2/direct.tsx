import {
  For,
  Repeat,
  children,
  createEffect,
  createSignal,
  onSettled,
} from "solid-js";

// The named-import twin of `App.tsx`: identical shapes, direct spellings.

const [items] = createSignal(["a"]);

createEffect(
  () => items(),
  () => {},
);

export function Leaf() {
  onSettled(() => {
    children(() => null);
  });
  return null;
}

export function Rows() {
  return <For each={items()}>{(item) => <div>{item}</div>}</For>;
}

export function Cells(props: { total: number }) {
  return <Repeat count={props.total}>{(index) => <div>{index}</div>}</Repeat>;
}
