import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Aliased() {
  const value = count();
  return <b>{value}</b>;
}

const Badge = Aliased;

export function App() {
  return (
    <div>
      <Aliased />
      <Badge />
    </div>
  );
}
