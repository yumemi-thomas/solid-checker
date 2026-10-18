import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";

function Kids(props: { children: (x: number) => JSX.Element }) {
  const el = props.children(1);
  return <div>{el}</div>;
}

export function ViaChildrenBody() {
  const [count] = createSignal(0);
  return (
    <Kids>
      {(x) => {
        console.log(count());
        return <span>{x}</span>;
      }}
    </Kids>
  );
}

function Runs(props: { render: (x: number) => JSX.Element }) {
  const el = props.render(1);
  return <div>{el}</div>;
}

export function ViaRenderPropBody() {
  const [count] = createSignal(0);
  return (
    <Runs
      render={(x) => {
        console.log(count());
        return <span>{x}</span>;
      }}
    />
  );
}
