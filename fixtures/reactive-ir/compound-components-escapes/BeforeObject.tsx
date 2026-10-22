import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Root() {
  return <i />;
}

function Sub() {
  const value = count();
  return <b>{value}</b>;
}

function mount(render: () => unknown) {
  return render();
}

// Uncertifiable: the same for a plain object holder; a try/catch around the
// early render does not make `Sub` run.
try {
  mount(() => <Parts.Sub />);
} catch {}

const Parts = { Sub };
