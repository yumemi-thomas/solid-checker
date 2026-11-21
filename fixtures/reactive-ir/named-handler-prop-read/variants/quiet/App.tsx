import { createSignal } from "solid-js";

// ADR 1146's twin: the same named handler with no dispatch anywhere, delivered only by the user (a delivery that sets `window.event`): a
// read in an event handler is legal there, so it stays clean.
type Props = { r: number };

function Cast(props: Props) {
  const f = () => { (props as typeof props).r; };
  return <div onScroll={f} />;
}

export function App() {
  const [r] = createSignal(1);
  return <Cast r={r()} />;
}
