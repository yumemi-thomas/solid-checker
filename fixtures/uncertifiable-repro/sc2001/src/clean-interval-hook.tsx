// Clean. Mirrors oscartbeaumont-website src/date.ts:3-10: a hook writes its signal from an interval
// started in onSettled and returns the stop function; the component reads the returned accessor.
import { createSignal, onSettled } from "solid-js";

function createDateNow(interval = 1_000) {
  const [now, setNow] = createSignal(new Date());
  onSettled(() => {
    const timer = setInterval(() => setNow(new Date()), interval);
    return () => clearInterval(timer);
  });
  return [now] as const;
}

export function Clock() {
  const [now] = createDateNow(50);
  return <time>{now().getFullYear()}</time>;
}
