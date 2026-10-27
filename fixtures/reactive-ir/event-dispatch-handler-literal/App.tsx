import { createRenderEffect, createSignal } from "solid-js";
import { withTx } from "./tx";

const [count, setCount] = createSignal(0);
const input = document.createElement("input");

export function Card() {
  return (
    <div>
      <button onClick={() => withTx(() => input.click())} />
      <button
        onClick={() => {
          setCount(count() + 1);
          createRenderEffect(() => count(), () => {});
        }}
      />
    </div>
  );
}
