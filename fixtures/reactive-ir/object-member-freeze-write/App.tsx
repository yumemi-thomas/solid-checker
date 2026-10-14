import { createSignal, onCleanup } from "solid-js";
// TypeScript keeps the real freeze signature; the runtime member was replaced.
Object.assign(Object, { freeze(value: unknown) {
  return { frozenRun() {}, frozenRead() { return 0; } };
} });
const [count] = createSignal(0);
const object = Object.freeze({
  frozenRun() { onCleanup(() => {}); },
  frozenRead: () => count(),
});
export function Refused() {
  const n = object.frozenRead();
  return <button onClick={async () => { await Promise.resolve(); object.frozenRun(); }}>{n}</button>;
}
