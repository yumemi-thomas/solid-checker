import { action, createComponent, createSignal, createStore } from "solid-js";
import { dynamic } from "@solidjs/web";
import { Holder, Label, Use, make, plain } from "dep-handed";

const Lazy = dynamic(() => Use);

export function Constructed() {
  const [, setN] = createSignal(0);
  new Holder(setN);
  return <p>x</p>;
}
export function Reflected() {
  const [, setN] = createSignal(0);
  Reflect.apply(plain, undefined, [setN]);
  return <p>x</p>;
}
export function Hopped() {
  const [, setN] = createSignal(0);
  return createComponent(Use, { set: setN });
}
export function Carried() {
  const [, setN] = createSignal(0);
  return <Lazy set={setN} />;
}
export function Held() {
  const [, setN] = createSignal(0);
  make(setN).value;
  return <p>x</p>;
}
export function Shorthand() {
  const [, set] = createSignal(0);
  return Use({ set });
}
export function Store() {
  const [state] = createStore({ v: 0 });
  return <Use store={state} />;
}
export function Accessor() {
  const [n] = createSignal(0);
  return <Use read={n} />;
}
export function Action() {
  const act = action(function* () {});
  return <Use act={act} />;
}
export function Walked() {
  const [, setN] = createSignal(0);
  plain(setN);
  return <p>x</p>;
}
export function Plain() {
  return <Label />;
}
