// app-game typegpu-examples 1.1-webgpu-fundamentals/index.tsx:81: a tracked effect calls a member of an object a package returned.
import { createCallbackStack } from "@solid-primitives/utils";
import { createSignal, createTrackedEffect } from "solid-js";

export function createRelay() {
  const stack = createCallbackStack<number>();
  const [count] = createSignal(0);
  createTrackedEffect(() => {
    stack.execute(count());
  });
  return stack;
}
