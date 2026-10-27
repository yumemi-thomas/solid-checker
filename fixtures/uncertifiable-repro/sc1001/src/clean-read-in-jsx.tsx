// Clean. Mirrors compass-ui apps/ui/src/components/BacklogView.tsx:65: signal, store and prop
// reads inside JSX are compiler-tracked; the same reads in the body are SC1001 (read-in-component-body).
import { createSignal, createStore } from "solid-js";

function Panel(props: { plan: string }) {
  const [open, setOpen] = createSignal(false);
  const [state] = createStore({ width: 4 });
  return (
    <button aria-expanded={open() ? "true" : "false"} onClick={() => setOpen(!open())}>
      {props.plan.trim()}
      {state.width * 2}
    </button>
  );
}

export function App() {
  const [plan] = createSignal("free");
  return <Panel plan={plan()} />;
}
