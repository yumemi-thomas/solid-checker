import {
  For,
  Repeat,
  Show,
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
  return <div />;
}

export function Rows() {
  return (
    <For each={items()}>
      {(item, index) => {
        const current = index();
        return <div>{current}</div>;
      }}
    </For>
  );
}

export function Cells(props: { total: number }) {
  return <Repeat count={props.total}>{(index) => <div>{index}</div>}</Repeat>;
}

export function Conditional() {
  return <Show when={items()}>{() => <div>{items()}</div>}</Show>;
}
