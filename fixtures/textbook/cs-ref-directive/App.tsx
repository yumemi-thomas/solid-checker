import { createEffect, createSignal, onSettled, type Accessor } from "solid-js";

function titled(source: Accessor<string>) {
  let target: HTMLElement | undefined;
  createEffect(source, (value) => {
    if (target) target.title = value;
  });
  return (el: HTMLElement) => {
    target = el;
  };
}

export function Focused() {
  let input: HTMLInputElement | undefined;
  const [hint] = createSignal("type here");
  onSettled(() => {
    input?.focus();
  });
  return <input ref={[(el) => (input = el), titled(hint)]} />;
}
