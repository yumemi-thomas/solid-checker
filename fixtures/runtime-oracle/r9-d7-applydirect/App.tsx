import { createSignal, createEffect, onSettled } from "solid-js";
function Child(props: { playingPath: () => string }) {
  createEffect(
    () => 1,
    () => {
      console.log(props.playingPath());
    },
  );
  return <span />;
}
export function C() {
  const [playing] = createSignal("");
  return <Child playingPath={() => playing() ?? ""} />;
}
