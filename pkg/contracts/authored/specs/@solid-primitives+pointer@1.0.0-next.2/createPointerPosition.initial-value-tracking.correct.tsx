import { createSignal } from "solid-js";
import { createPointerPosition, type PointerStateWithActive } from "@solid-primitives/pointer";
const initial: PointerStateWithActive = {
  x: 0, y: 0, pointerId: 0, pressure: 0, tiltX: 0, tiltY: 0,
  width: 0, height: 0, twist: 0, pointerType: null, isActive: false
};
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  const value = Object.assign(() => {
    source();
    // Write sink in the event handler only.
    return initial;
  }, initial);
  createPointerPosition({ value });
  const finish = () => {
    setSink(1);
    setDone("done");
  };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
