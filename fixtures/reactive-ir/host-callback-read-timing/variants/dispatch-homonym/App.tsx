import * as solid from "solid-js";
const bus = { dispatchEvent(_event: Event) { return true; }, click() {} };
export function App() {
  bus.dispatchEvent(new Event("click"));
  bus.click();
  return <button onClick={() => solid.onCleanup(() => {})} />;
}
