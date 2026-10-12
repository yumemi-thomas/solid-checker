import { createEffect, createMemo, createSignal } from "solid-js";
import { useValue } from "./context";
export function Body() {
  const value = useValue();
  const orientation = value.orientation();
  return <p>{orientation}</p>;
}
export function Apply() {
  const value = useValue();
  createEffect(() => 1, () => { void value.orientation(); });
  return <p>apply</p>;
}
export function Eager() {
  const value = useValue();
  const [orientation] = createSignal(value.orientation());
  return <p>{orientation()}</p>;
}
export function Tracked() {
  const value = useValue();
  const orientation = createMemo(() => value.orientation());
  return <p>{orientation()}</p>;
}

import * as api from "./context";
export function NamespaceWrapped() {
  const value = (api.useValue() satisfies ReturnType<typeof api.useValue>);
  const orientation = (value.orientation as () => number)();
  return <p>{orientation}</p>;
}
