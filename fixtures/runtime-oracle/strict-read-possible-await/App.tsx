import { createSignal } from "solid-js";

function now(callback: () => void) { callback(); }
const pause = () => new Promise<void>(resolve => setTimeout(resolve, 10));

// No warning: each read runs after the strict-read window has closed.
export function ConditionalSuspends() {
  const [count] = createSignal(1);
  now(async () => {
    if (true) await pause();
    document.querySelector("#conditional-result")!.textContent = String(count());
  });
  return <output id="conditional-result">waiting</output>;
}

export function LoopSuspends() {
  const [count] = createSignal(2);
  now(async () => {
    for (const item of [1]) { await pause(); void item; }
    document.querySelector("#loop-result")!.textContent = String(count());
  });
  return <output id="loop-result">waiting</output>;
}

export function FinallySuspends() {
  const [count] = createSignal(3);
  now(async () => {
    try { await pause(); }
    finally {
      document.querySelector("#finally-result")!.textContent = String(count());
    }
  });
  return <output id="finally-result">waiting</output>;
}

// Warning: this read still runs synchronously in the render window.
export function BeforeAwaitControl() {
  const [count] = createSignal(4);
  now(async () => {
    console.log(count());
    await pause();
  });
  return <output>control</output>;
}

// Warning: the read cannot follow the other arm's await.
export function ExclusiveArmControl() {
  const [count] = createSignal(5);
  now(async () => {
    if (false) await pause();
    else console.log(count());
  });
  return <output>exclusive</output>;
}
