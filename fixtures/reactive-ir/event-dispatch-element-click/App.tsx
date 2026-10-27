import { createRenderEffect, createSignal, runWithOwner } from "solid-js";

const [count, setCount] = createSignal(0);

// The subject: a handler on an intrinsic element. Whether its write and its
// read are proven event-only, and its effect proven ownerless, is what the
// dispatch below decides.
export function Card() {
  const el = document.createElement("input");
  const poke = () => el.click();
  poke();
  return (
    <button
      onClick={() => {
        setCount(count() + 1);
        createRenderEffect(() => count(), () => {});
      }}
    />
  );
}
