import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";
type Props = { children: () => JSX.Element; foo: number };
function replace(target: unknown) {
  if (target) Reflect.set(target as object, "children", () => <span>replacement</span>);
  return 0;
}
function Kids(props: Props) {
  const foo = props.foo;
  return props.children();
}
export function App(this: Props) {
  const [n] = createSignal(0);
  return <Kids foo={replace(this)}>{() => { const value = n(); return <span>{value}</span>; }}</Kids>;
}
