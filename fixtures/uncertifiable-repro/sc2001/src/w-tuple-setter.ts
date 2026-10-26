// hoxxes-briefing create-local-storage.ts:29-37: the local `set` that writes is returned inside an
// array (`return [value, set]`).
import { createSignal } from "solid-js";

export function createLocalValue(key: string) {
  const [value, setValue] = createSignal(localStorage.getItem(key) ?? "");
  const set = (next: string) => {
    setValue(() => next);
    localStorage.setItem(key, next);
  };
  return [value, set] as const;
}
