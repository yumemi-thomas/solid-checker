import { createSignal } from "solid-js";
import runIt from "./helper";

// The closure `runIt` returns is called only in an event.
export function DefaultExported() {
  const [count] = createSignal(0);
  const run = runIt(() => console.log(count()));
  return <button onClick={() => run()}>a</button>;
}

// A component referenced as a value as well as rendered.
function Row(props: { onX: () => void }) {
  return <button onClick={props.onX}>row</button>;
}
export const routes = [{ path: "/", component: Row }];
export function AsValue() {
  const [count] = createSignal(0);
  return <Row onX={() => console.log(count())} />;
}

// A local function whose returned closure is called in the body of a
// function exported by `export default` expression.
function makeRun(cb: () => void) {
  return () => cb();
}
function runner() {
  const r = makeRun(() => undefined);
  r();
}
export default runner;
export function LocalViaDefault() {
  const [count] = createSignal(0);
  const r = makeRun(() => console.log(count()));
  return <button onClick={() => r()}>b</button>;
}
