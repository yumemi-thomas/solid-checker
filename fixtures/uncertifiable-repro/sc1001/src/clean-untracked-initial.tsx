// Clean. Mirrors spookysoftware-www domains/www/src/terminal.tsx:20-21 and error-menu-web
// AnalyzerCard.tsx:35: a prop read as a signal's initial value, said to be a one-time read with untrack
// (the bare read is SC1001, prop-as-signal-initial).
import { createSignal, untrack } from "solid-js";

function Menu(props: { x: number; y: number }) {
  const [position, setPosition] = createSignal(untrack(() => ({ x: props.x, y: props.y })));
  return <button onClick={() => setPosition({ x: 0, y: 0 })}>{position().x}</button>;
}

export function App() {
  const [x] = createSignal(1);
  return <Menu x={x()} y={2} />;
}
