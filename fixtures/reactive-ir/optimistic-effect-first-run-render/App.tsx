import { createOptimistic, createRenderEffect } from "solid-js";

// ADR 1224: a render effect's apply runs at its call, in the child's body on the creating pass: rc.21 throws
// OPTIMISTIC_WRITE_BEFORE_LOAD at the setter. A violation.
function Kid(props: { run: () => void }) {
  createRenderEffect(
    () => 1,
    () => {
      props.run();
    },
  );
  return <p />;
}

export function App() {
  const [, setO] = createOptimistic(async () => 1);
  return (
    <div>
      <Kid run={() => setO(3)} />
    </div>
  );
}
