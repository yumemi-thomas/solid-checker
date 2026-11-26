import { createRenderEffect, createSignal } from "solid-js";

// ADR 1227: the child's render effect runs its first apply at its call, under the child's owner, while the mount's
// creation pass renders it (a direct child of intrinsic elements; its props do not matter): the write throws
// REACTIVE_WRITE_IN_OWNED_SCOPE on rc.21. A violation.
function Child(props: { set: (value: number) => number; extra: number }) {
  createRenderEffect(
    () => 1,
    (v) => {
      props.set(v);
    },
  );
  return <p />;
}

export function App() {
  const [, setW] = createSignal(0);
  return (
    <div>
      <Child set={setW} extra={1} />
    </div>
  );
}
