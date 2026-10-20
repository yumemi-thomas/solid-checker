import { createSignal, createEffect, onSettled } from "solid-js";
function Child(props: { playingPath: () => string }) {
  function updateVisibility() {
    console.log(props.playingPath());
  }
  createEffect(
    () => 1,
    () => {
      updateVisibility();
    },
  );
  return <span />;
}
export function C() {
  const [playing] = createSignal("");
  return <Child playingPath={() => playing() ?? ""} />;
}
