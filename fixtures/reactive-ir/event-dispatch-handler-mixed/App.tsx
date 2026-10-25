import { createRenderEffect, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);
const input = document.createElement("input");

// `press` is both an event handler and called from the component body, so not
// every invocation is an event delivery: the dispatch in it can run on the
// body's owned stack and keeps the veto. (ADR 0327 refuses the event-only
// proof for a function with a mixed invocation.)
export function Card() {
  const press = () => input.click();
  press();
  return (
    <div>
      <button onClick={press} />
      <button
        onClick={() => {
          setCount(count() + 1);
          createRenderEffect(() => count(), () => {});
        }}
      />
    </div>
  );
}
