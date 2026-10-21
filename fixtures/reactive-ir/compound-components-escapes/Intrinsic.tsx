import { createSignal } from "solid-js";

const [count] = createSignal(0);

function NameRoot() {
  return <i />;
}

function LengthRoot() {
  return <i />;
}

function NameSub() {
  const value = count();
  return <b>{value}</b>;
}

function LengthSub() {
  const value = count();
  return <u>{value}</u>;
}

// Uncertifiable: a function's own `name` and `length` are non-writable, so
// `Object.assign(NameRoot, { name: … })` throws a TypeError and nothing renders
// (`prototype`, `arguments` and `caller` are refused too).
export function Screen() {
  try {
    const Tabs = Object.assign(NameRoot, { name: NameSub });
    return <Tabs.name />;
  } catch {
    return <i />;
  }
}

export function Other() {
  try {
    const Tabs = Object.assign(LengthRoot, { length: LengthSub });
    return <Tabs.length />;
  } catch {
    return <i />;
  }
}
