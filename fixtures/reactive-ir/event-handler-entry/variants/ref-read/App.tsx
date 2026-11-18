import { resolve } from "solid-js";

// Open: code holding the element reads the handler Solid stored on it (`_$$click`), and may call it anywhere.
export function Resolve() {
  let el!: HTMLButtonElement;
  queueMicrotask(() => {
    (el as unknown as { _$$click?: () => void })._$$click?.();
  });
  return <button ref={el} onClick={() => { resolve(() => 1); }}>go</button>;
}
