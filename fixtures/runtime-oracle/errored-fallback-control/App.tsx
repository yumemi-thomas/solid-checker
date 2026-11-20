import { createSignal, onCleanup } from "solid-js";

export function Probe() {
  const [n, setN] = createSignal(0);
  return (
    <button
      type="button"
      id="go"
      onClick={() => {
        onCleanup(() => {});
        setN((v) => v + 1);
      }}
    >
      {n()}
    </button>
  );
}
