import { createMemo, createSignal } from "solid-js";
import { Bump, Label, SetBy, bump, bumpNow, reset, store } from "dep-write";

export function CalledInMemo() {
  const m = createMemo(() => bump());
  return <p>{String(m())}</p>;
}
export function GetterInMemo() {
  const m = createMemo(() => store.bumped);
  return <p>{m()}</p>;
}
export function CalledInBody() {
  bumpNow();
  return <p>x</p>;
}
export function Rendered() {
  return <Bump />;
}
export function PropSetter() {
  const [, setN] = createSignal(0);
  return <SetBy set={setN} />;
}
export function Handler() {
  return <button onClick={() => reset()}>reset</button>;
}
export function ObjectSetter() {
  const [, setN] = createSignal(0);
  return SetBy({ set: setN });
}
export function ArraySetter() {
  const [, setN] = createSignal(0);
  return SetBy({ set: [setN][0] });
}
export function ClosureSetter() {
  const [, setN] = createSignal(0);
  return <SetBy set={(value) => setN(value)} />;
}
export function Plain() {
  return <Label />;
}
