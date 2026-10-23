import { Dynamic } from "@solidjs/web";
import { createSignal, For } from "solid-js";

const [count] = createSignal(0);

function Listed() {
  return (
    <For each={[1]}>
      {(item) => {
        const value = count();
        return <b id="rendered">rendered {value}:{item}</b>;
      }}
    </For>
  );
}

export function App() {
  return <Dynamic component={Listed} />;
}
