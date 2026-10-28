import { createRenderEffect, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);

// The subject: a handler on an intrinsic element. Whether its write and its
// read are proven event-only, and its effect proven ownerless, is what the
// dispatch below decides.
export function Card() {
  return (
    <button
      onClick={() => {
        setCount(count() + 1);
        createRenderEffect(() => count(), () => {});
      }}
    />
  );
}

const pause = () => new Promise<void>((done) => setTimeout(done, 0));
// The dispatch is in a closure written after the await: whoever calls the
// closure runs it on their own stack.
export async function exercise() {
  await pause();
  return () => document.createElement("input").dispatchEvent(new Event("input"));
}
