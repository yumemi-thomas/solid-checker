import { createSignal } from "solid-js";
function useNow() {
  const [now, setNow] = createSignal(1);
  return [now] as const;
}
export function Settled() {
  const [now] = useNow();
  return <time>{now()}</time>;
}
