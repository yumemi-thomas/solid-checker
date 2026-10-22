import { createEffect, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);

// Read only from View.tsx's JSX: where it runs is View's compiler fact.
export function readCount() {
  return count();
}

// Read from View.tsx's JSX and directly in App's component body.
export function readShared() {
  return count();
}

// Called only in View.tsx's tracked JSX.
export function bump() {
  setCount(1);
}

// Called only from View.tsx's event handler.
export function useTicker() {
  createEffect(
    () => count(),
    () => {}
  );
}
