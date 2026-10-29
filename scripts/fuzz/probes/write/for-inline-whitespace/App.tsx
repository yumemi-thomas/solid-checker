// ADR 0508 review (false positive): inline whitespace beside a <For> row makes props.children an array, so mapArray throws and the row never renders; no write runs (tsc rejects the children, TS2746).
import { For, createRenderEffect, createSignal } from "solid-js";

function C1() {
  const [w1, setW1] = createSignal(0);
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    setW1(7);
  });
  return (
    <div>{w1()}</div>
  );
}

export function P1() {
  return (
    <div>
      {/* @ts-ignore */}
      <For each={[1]}> {() => <C1 />} </For>
    </div>
  );
}
