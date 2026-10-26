// app-game typegpu-examples 3-webgpu-uniforms/index.tsx:159: a prop is read in a callback a package keeps, inside a tracked scope.
import { createCallbackStack } from "@solid-primitives/utils";
import { createMemo } from "solid-js";

export function Overlay(props: { amount: number }) {
  const stack = createCallbackStack<number>();
  const total = createMemo(() => {
    stack.push((value) => console.log(value + props.amount));
    return props.amount;
  });
  return <canvas width={total()} />;
}
