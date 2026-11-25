import { createRenderEffect, createSignal } from "solid-js";

// ADR 1227: the child is rendered in another component's children, which no fact places on the creation pass:
// uncertifiable (rc.21 throws when `Wrap` renders them: a recall miss).
function Child(props: { set: (value: number) => number; extra: number }) {
  createRenderEffect(
    () => 1,
    (v) => {
      props.set(v);
    },
  );
  return <p />;
}

function Wrap(props: { children?: unknown }) {
  return <div>{props.children as never}</div>;
}

export function App() {
  const [, setW] = createSignal(0);
  return (
    <Wrap>
      <Child set={setW} extra={1} />
    </Wrap>
  );
}
