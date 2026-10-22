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

function build() {
  return mount(() => <Tabs.Sub />);
}

// Uncertifiable: an earlier declaration's initializer is a call and may render.
const early = build();

const Tabs = Object.assign(Root, { Sub });
