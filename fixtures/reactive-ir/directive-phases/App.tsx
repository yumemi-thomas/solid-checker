import { createSignal } from "solid-js";

const [count, setCount] = createSignal(0);

function directive() {
  setCount(1);
  return element => {
    setCount(2);
    createSignal(element);
  };
}

function innerDirective() {
  return element => createSignal(element);
}

function forwardedDirective() {
  return innerDirective();
}

function makeHandler() {
  setCount(4);
  return () => {};
}

export function App() {
  return <button
    ref={[directive(), forwardedDirective(), element => {
      setCount(3);
      createSignal(element);
    }]}
    onClick={makeHandler()}
  >{count()}</button>;
}
