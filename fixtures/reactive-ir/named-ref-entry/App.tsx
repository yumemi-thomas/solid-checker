import { createSignal } from "solid-js";

const [w, setW] = createSignal(0);

// ADR 1160: a function whose only reference is an intrinsic element's `ref={name}` is entered by the ref's application alone,
// which runs untracked and outside any owner the setter guard refuses: the write is silent (Chrome quiet).
export function RefWrite() {
  const rf1 = () => {
    setW(1);
  };
  return <p ref={rf1}>a</p>;
}

// The same function also called in the component body: that run writes during setup, under the component's owner.
export function AlsoCalled() {
  const rf2 = () => {
    setW(2);
  };
  rf2();
  return <p ref={rf2}>b</p>;
}

// Exported: an importer the program does not show may call it anywhere.
export const rf3 = () => {
  setW(3);
};
export function Exported() {
  return <p ref={rf3}>c</p>;
}

// A component's `ref` is a prop: the child decides when it runs.
function Kid(props: { ref: () => void }) {
  props.ref();
  return <i />;
}
export function ComponentRef() {
  const rf4 = () => {
    setW(4);
  };
  return <Kid ref={rf4} />;
}

// A read in the ref's application: no tracking scope, the dev build warns STRICT_READ_UNTRACKED.
export function RefRead() {
  const rf5 = () => {
    w();
  };
  return <p ref={rf5}>e</p>;
}
