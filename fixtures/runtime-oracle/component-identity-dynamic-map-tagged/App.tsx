import { Dynamic } from "@solidjs/web";
import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Archive() {
  const value = count();
  return <b id="rendered">rendered {value}</b>;
}

const icons = { archive: Archive };

export function App() {
  return (
    <div>
      <Archive />
      <Dynamic component={icons.archive} />
    </div>
  );
}
