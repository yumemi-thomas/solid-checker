import { createEffect, createOptimistic } from "solid-js";

// ADR 1224: the prop is a function the parent declares, handed by name: the child's effect apply calls it in `render`'s
// tail flush. A violation.
function Kid(props: { run: () => void }) {
  createEffect(
    () => 1,
    () => {
      props.run();
    },
  );
  return <p />;
}

export function App() {
  const [, setO] = createOptimistic(async () => 1);
  function run() {
    setO(7);
  }
  return (
    <div>
      <Kid run={run} />
    </div>
  );
}
