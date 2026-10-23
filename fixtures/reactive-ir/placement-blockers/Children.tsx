import { createSignal } from "solid-js";

declare function makeHandler(value: number): () => void;

function Frame(props: { children?: unknown }) {
  return <div>{props.children}</div>;
}

// The handler factory call runs when `Frame` evaluates its children, not in
// this body's strict-read window: SC1001 uncertifiable, blocker
// `component-children`.
export function InChildren() {
  const [count] = createSignal(0);
  return (
    <Frame>
      <button onClick={makeHandler(count())}>x</button>
    </Frame>
  );
}

// Control: the same call on an intrinsic element outside any component's
// children runs in the body's window, a proven violation with no blocker.
export function OutsideChildren() {
  const [count] = createSignal(0);
  return <button onClick={makeHandler(count())}>x</button>;
}
