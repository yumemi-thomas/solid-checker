import { createSignal } from "solid-js";
import { log } from "plain-log";

// SC9012 `package-callee`: a package function receives a value,
// not the accessor; the accessor goes to a project function or an ambient
// global, which this rule does not cover.
function show(read: () => number) {
  return read();
}
export function Sources() {
  const [count] = createSignal(0);
  setTimeout(() => log(count()), 0);
  return <div onClick={() => log(count())}>{show(count)}</div>;
}
