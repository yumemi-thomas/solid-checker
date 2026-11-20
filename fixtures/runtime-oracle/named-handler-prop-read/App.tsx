import { createEffect, createSignal } from "solid-js";

// ADR 1146: a prop read in a named handler that an effect's apply delivers into a shadow tree runs inside the apply's strict-read window:
// STRICT_READ_UNTRACKED (the cast and the plain read alike). The quiet twin is never dispatched: nothing.
type Props = { r: number };

function deliver(box: HTMLElement) {
  document.createElement("div").attachShadow({ mode: "open" }).appendChild(box);
  createEffect(() => 1, () => { box.dispatchEvent(new Event("scroll")); });
}

function Cast(props: Props) {
  const f = () => { (props as typeof props).r; };
  deliver((<div onScroll={f} />) as HTMLDivElement);
  return <i />;
}
function Plain(props: Props) {
  const f = () => { props.r; };
  deliver((<div onScroll={f} />) as HTMLDivElement);
  return <i />;
}
function Quiet(props: Props) {
  const f = () => { (props as typeof props).r; };
  return <div onScroll={f} />;
}

export function App() {
  const [r] = createSignal(1);
  return (
    <div>
      <Cast r={r()} />
      <Plain r={r()} />
      <Quiet r={r()} />
    </div>
  );
}
