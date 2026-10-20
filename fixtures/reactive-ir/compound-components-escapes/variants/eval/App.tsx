import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Sub() {
  const value = count();
  return <span>{value}</span>;
}

function read() {
  return count();
}

const Tabs = { Sub };
const Helped = {
  Sub: () => {
    const value = read();
    return <b>{value}</b>;
  },
};

// Uncertifiable: a direct `eval` can replace any member
// (`Tabs.Sub = () => null`), so no holder is a census anywhere in the project.
eval("Tabs.Sub = () => null");

export function Screen() {
  return (
    <>
      <Tabs.Sub />
      <Helped.Sub />
    </>
  );
}
