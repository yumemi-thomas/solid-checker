// app-game gsap/Animations.tsx:111: an array literal in the `each` of Solid's own `<For>` holds members that read a
// signal (`fn: () => linear(time())`); the loop's callback calls them in its JSX.
import { createSignal, For } from "solid-js";

const linear = (value: number) => value;
const quad = (value: number) => value * value;

export function Easings() {
  const [time, setTime] = createSignal(0);
  return (
    <>
      <input type="range" onInput={(event) => setTime(Number(event.currentTarget.value))} />
      <For
        each={[
          { name: "linear", fn: () => linear(time()) },
          { name: "quad", fn: () => quad(time()) },
        ]}
      >
        {(item) => (
          <p>
            {item.name}: {item.fn()}
          </p>
        )}
      </For>
    </>
  );
}
