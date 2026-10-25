import { createRenderEffect, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);
const input = document.createElement("input");

// Two handlers on intrinsic elements. The first dispatches synchronously, but
// only from inside an event handler, which runs with no owner, so what it
// delivers runs with none either; the second's write and effect creation are
// the subject.
export function Card() {
  return (
    <div>
      <button onClick={() => input.click()} />
      <button
        onClick={() => {
          setCount(count() + 1);
          createRenderEffect(() => count(), () => {});
        }}
      />
    </div>
  );
}
