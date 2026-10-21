import { createEffect, createSignal, onCleanup, onSettled } from "solid-js";

const [count] = createSignal(0);

export function fnA() {
  onSettled(() => { onCleanup(() => {}); });
}

export function fnB() {
  onSettled(() => { createEffect(count, () => {}); });
}

export function fnC() {
  onSettled(() => { onCleanup(() => {}); });
}
