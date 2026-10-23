import { Dynamic } from "@solidjs/web";
import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Panel() {
  const value = count();
  return <b id="rendered">rendered {value}</b>;
}

export function App() {
  return (
    <div>
      <Dynamic component={Panel} />
    </div>
  );
}
