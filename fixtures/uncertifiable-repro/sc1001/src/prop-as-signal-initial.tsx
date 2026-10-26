// Violation. Corpus: app-game apps/browser-atlas/src/App.tsx:2802, kui feature-clusters/BrokersScreen.tsx:127,
// viviana-ui-web components/theme/ColorKnob.tsx:34, probus-hk routes/Search.tsx:302: a prop
// read as a signal's initial value is taken once, when the component is created.
import { createSignal } from "solid-js";

function Menu(props: { x: number; y: number }) {
  const [position, setPosition] = createSignal({ x: props.x, y: props.y });
  return <button onClick={() => setPosition({ x: 0, y: 0 })}>{position().x}</button>;
}

export function App() {
  const [x] = createSignal(1);
  return <Menu x={x()} y={2} />;
}
