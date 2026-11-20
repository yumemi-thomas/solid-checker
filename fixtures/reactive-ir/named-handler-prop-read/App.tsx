import { createEffect, createSignal } from "solid-js";

// ADR 1146: a prop read in a handler named by its value (`onScroll={f}`), in a program where an effect's apply dispatches into a shadow
// tree: the listener runs with `window.event` unset, inside the apply's strict-read window (Chrome rc.21: STRICT_READ_UNTRACKED). The
// read stays uncertifiable, as it is in an inline handler, whatever TypeScript wrapper the receiver has.
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
function NonNull(props: Props) {
  const f = () => { props!.r; };
  deliver((<div onScroll={f} />) as HTMLDivElement);
  return <i />;
}
function Satisfies(props: Props) {
  const f = () => { (props satisfies Props).r; };
  deliver((<div onScroll={f} />) as HTMLDivElement);
  return <i />;
}
function Paren(props: Props) {
  const f = () => { (props).r; };
  deliver((<div onScroll={f} />) as HTMLDivElement);
  return <i />;
}
function Plain(props: Props) {
  const f = () => { props.r; };
  deliver((<div onScroll={f} />) as HTMLDivElement);
  return <i />;
}

export function App() {
  const [r] = createSignal(1);
  return (
    <div>
      <Cast r={r()} />
      <NonNull r={r()} />
      <Satisfies r={r()} />
      <Paren r={r()} />
      <Plain r={r()} />
    </div>
  );
}
