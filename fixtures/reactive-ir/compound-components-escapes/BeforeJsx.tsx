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

// Uncertifiable: a JSX element written before the holder is code that runs
// there, and the analysis does not tell an intrinsic element from a component
// that names the holder. (A primitive given only data, `createSignal(0)`
// above, runs no user code and withholds nothing.)
const node = <i />;

const Parts = { Sub };

export function Screen() {
  return <Parts.Sub />;
}
