import { createEffect, createSignal } from "solid-js";

// A body's window takes any event: a listener it dispatches reads in it. The
// run through the listener is followed by nothing: open.
export function DispatchedFromBody() {
  const [count] = createSignal(1);
  const element = document.createElement("button");
  element.addEventListener("click", () => {
    console.log(count());
  });
  element.click();
  return <p>{1}</p>;
}

// An apply's window records its queue's event: a handler it dispatches reads
// under another event, in no window.
export function DispatchedFromApply() {
  const [count] = createSignal(1);
  let button!: HTMLButtonElement;
  createEffect(
    () => 1,
    () => {
      button.click();
    },
  );
  return (
    <button ref={button} onClick={() => console.log(count())}>
      x
    </button>
  );
}

// What a handler or an apply schedules runs in no window.
export function Scheduled() {
  const [count] = createSignal(1);
  createEffect(
    () => 1,
    () => {
      setTimeout(() => console.log(count()), 0);
      queueMicrotask(() => console.log(count()));
    },
  );
  return (
    <button
      onClick={async () => {
        await Promise.resolve();
        console.log(count());
        queueMicrotask(() => console.log(count()));
      }}
    >
      x
    </button>
  );
}

// The apply's own read is in its window, after a dispatch too.
export function ApplyRead() {
  const [count] = createSignal(1);
  let button!: HTMLButtonElement;
  createEffect(
    () => 1,
    () => {
      button.click();
      console.log(count());
    },
  );
  return <button ref={button}>x</button>;
}
