import { createSignal, onSettled } from "solid-js";
function createDateNow(interval = 1_000) {
  const [now, setNow] = createSignal(new Date());
  onSettled(() => {
    const timer = setInterval(() => setNow(new Date()), interval);
    return () => clearInterval(timer);
  });
  return [now] as const;
}
export function SettledInterval() {
  const [now] = createDateNow(50);
  return <time>{now().getFullYear()}</time>;
}
