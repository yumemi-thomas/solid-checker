import { createRenderEffect, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);
const input = document.createElement("input");
const other = document.createElement("input");

// Two dispatch sites. `refresh` is called only from the first handler, so
// every invocation of it is an event delivery too; the second handler's
// dispatch is written directly in the handler.
function refresh(target: HTMLElement) {
  target.click();
}

export function Card() {
  return (
    <div>
      <button onClick={() => refresh(input)} />
      <button onClick={() => other.click()} />
      <button
        onClick={() => {
          setCount(count() + 1);
          createRenderEffect(() => count(), () => {});
        }}
      />
    </div>
  );
}
