// Violation. Corpus: app-game math-examples/rapier-2d-physics-engine/index.tsx:40 (store),
// compass-ui Bridge.tsx:357 (signal), donegeon-client WaitlistCard.tsx:25 (prop). A reactive
// read in the body of a component, outside JSX, runs once and never updates.
import { createSignal, createStore } from "solid-js";

function Panel(props: { plan: string }) {
  const [tab, setTab] = createSignal("a");
  const [state] = createStore({ width: 4 });
  const label = props.plan.trim();
  const first = tab();
  const box = state.width * 2;
  return (
    <button onClick={() => setTab("b")}>
      {label}
      {first}
      {box}
    </button>
  );
}

export function App() {
  const [plan] = createSignal("free");
  return <Panel plan={plan()} />;
}
