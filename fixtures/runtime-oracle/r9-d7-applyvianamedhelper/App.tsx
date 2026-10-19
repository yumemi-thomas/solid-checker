import { createSignal, createEffect, onSettled } from "solid-js";
function Child(props: { playingPath: () => string }) {
  function updateVisibility() {
    const path = props.playingPath();
    console.log(path);
  }
  createEffect(
    () => props.playingPath(),
    () => {
      updateVisibility();
    },
  );
  onSettled(() => {
    window.addEventListener("scroll", updateVisibility);
    updateVisibility();
  });
  return <span />;
}
export function C() {
  const [playing] = createSignal("");
  return <Child playingPath={() => playing() ?? ""} />;
}
