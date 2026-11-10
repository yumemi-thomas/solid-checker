import { createSignal, untrack } from "solid-js";

// ADR 0937: module bindings named `addEventListener` and `dispatchEvent` are not lib.dom's globals. Nothing registers or dispatches,
// the listener never runs, and Chrome reports nothing: no `listener-reach` obligation.
const addEventListener = (_type: string, _listener: () => void) => {};
const dispatchEvent = (_event: Event) => true;

export function Shadowed() {
  const [r1] = createSignal(3);
  untrack(() => {
    const tf1 = () => {
      r1();
    };
    addEventListener("sc-evt", tf1);
  });
  dispatchEvent(new Event("sc-evt"));
  return <p />;
}
