import { createEffect, createOptimistic } from "solid-js";

// ADR 1224: the child is rendered inside another component's children, not as a direct child of intrinsic elements: no
// proof of when it renders. Uncertifiable.
function Kid(props: { run: () => void }) {
  createEffect(
    () => 1,
    () => {
      props.run();
    },
  );
  return <p />;
}

function Wrap(props: { children?: unknown }) {
  return <div>{props.children as never}</div>;
}

export function App() {
  const [, setO] = createOptimistic(async () => 1);
  return (
    <Wrap>
      <Kid run={() => setO(5)} />
    </Wrap>
  );
}
