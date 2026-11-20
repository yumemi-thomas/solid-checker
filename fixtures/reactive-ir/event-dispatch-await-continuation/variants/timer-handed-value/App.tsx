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

// ADR 1125: the timer gets a value the facts do not prove a function (a copy of the resolver): it may run a string, so
// every event proof is withheld and the read after the await stays open.
const pause = () =>
  new Promise<void>((done) => {
    const later: unknown = done;
    setTimeout(later as () => void, 0);
  });
async function exercise() {
  await pause();
  for (const element of Array.from(document.querySelectorAll("*"))) element.dispatchEvent(new Event("click", { bubbles: true }));
}
void exercise();
