import { createSignal } from "solid-js";
function useX() {
  const [s] = createSignal(1);
  return [s] as const;
}
export function BodyRead() {
  const [m] = useX();
  const v = m();
  return <time>{v}</time>;
}
