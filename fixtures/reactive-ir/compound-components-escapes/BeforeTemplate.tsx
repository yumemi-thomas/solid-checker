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

function tag(_strings: TemplateStringsArray, ..._values: unknown[]) {
  return mount(() => <Parts.Sub />);
}

// Uncertifiable: a tagged template before the holder calls its tag.
const text = tag`before`;

const Parts = { Sub };
