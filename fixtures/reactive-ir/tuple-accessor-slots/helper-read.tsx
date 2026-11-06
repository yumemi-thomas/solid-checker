import { createSignal } from "solid-js";
function useX() {
  const [s] = createSignal(1);
  return [s] as const;
}
function readIt() {
  const [m] = useX();
  return m();
}
export function HelperRead() {
  const v = readIt();
  return <time>{v}</time>;
}
