import { createSignal } from "solid-js";

const [count] = createSignal(0);

// Uncertifiable: `renderEarly` runs before `const Sub` and `const Root` are
// initialized, so each holder's initializer throws a ReferenceError and no
// member is ever rendered. A holder is a census only at module top level, with
// its values and `Object.assign` target declared earlier there.
try {
  renderEarlyValue();
} catch {}
try {
  renderEarlyTarget();
} catch {}

const Sub = () => {
  const value = count();
  return <b>{value}</b>;
};

const Root = () => <i />;

function Late() {
  const value = count();
  return <u>{value}</u>;
}

function renderEarlyValue() {
  const Tabs = { Sub };
  return <Tabs.Sub />;
}

function renderEarlyTarget() {
  const Tabs = Object.assign(Root, { Late });
  return <Tabs.Late />;
}
