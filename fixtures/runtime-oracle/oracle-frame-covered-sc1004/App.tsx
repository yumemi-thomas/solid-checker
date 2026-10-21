// Pins staticOnly's frame coverage. The SC1004 finding sits at the props read in
// Child. The runtime's diagnostic for that read is attributed to the call site in
// Parent, and the read's line is only one of its application frames. Before frame
// coverage, the finding was printed as static-only even though the diagnostic
// covers it. The case's verdict is covered-violation and its staticOnly is empty.
import { createSignal } from "solid-js";

function Child(props: { ready: boolean }) {
  if (!props.ready) return <p>waiting</p>;
  return <p>ready</p>;
}

export function Parent() {
  const [ready] = createSignal(false);
  return (
    <main>
      <Child ready={ready()} />
    </main>
  );
}
