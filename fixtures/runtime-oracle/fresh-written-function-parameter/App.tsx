import { createSignal } from "solid-js";

// ADR 1041 negative: the written object is a parameter, and the caller hands it `Number`: `Number.isFinite` is replaced by a
// function that writes a signal, so the call in the component body writes it. Chrome rc.21 reports
// REACTIVE_WRITE_IN_OWNED_SCOPE; the checker keeps the call uncertifiable (`standard-member-impostor`), never certified.
export function decorate(fn: any, key: string, value: unknown) {
  fn[key] = value;
}

const [w, setW] = createSignal(0);
decorate(Number, "isFinite", () => {
  setW(1);
  return true;
});

export function C1() {
  Number.isFinite(3);
  return <p>{w()}</p>;
}
