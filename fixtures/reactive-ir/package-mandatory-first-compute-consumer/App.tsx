import { createSignal } from "solid-js";
import { eagerTracked, possibleTracked, lazyTracked } from "reactive-package";

export function Correct() {
  const [source] = createSignal(1);
  eagerTracked(() => source());
  return <div />;
}

export function Misuse() {
  const [source, setSource] = createSignal(1);
  eagerTracked(() => { setSource(2); return source(); });
  return <div />;
}

export function ZeroIsStillZero() {
  const [source, setSource] = createSignal(1);
  possibleTracked(() => { setSource(2); return source(); });
  lazyTracked(() => { setSource(2); return source(); });
  return <div />;
}
