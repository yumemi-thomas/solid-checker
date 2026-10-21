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

// Uncertifiable: a statement before the holder calls a function that renders
// `<Tabs.Sub />`. `Tabs` is still uninitialized then, the tag throws a
// ReferenceError and `Sub` never runs, so the read in it is not proven.
mount(() => <Tabs.Sub />);

const Tabs = Object.assign(Root, { Sub });
