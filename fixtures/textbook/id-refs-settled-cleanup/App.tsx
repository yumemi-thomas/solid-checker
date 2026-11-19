import { createSignal, onCleanup, onSettled } from "solid-js";

export function Clock() {
  let box: HTMLDivElement | undefined;
  const [width, setWidth] = createSignal(0);
  const [ticks, setTicks] = createSignal(0);
  onSettled(() => {
    setWidth(box?.getBoundingClientRect().width ?? 0);
  });
  const timer = setInterval(() => setTicks((n) => n + 1), 1000);
  onCleanup(() => clearInterval(timer));
  return (
    <div ref={(el) => (box = el)}>
      {width()} {ticks()}
    </div>
  );
}
