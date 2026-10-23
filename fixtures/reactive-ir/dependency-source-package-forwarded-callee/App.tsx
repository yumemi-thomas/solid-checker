import { createSignal } from "solid-js";
import { aliased, chained, destructured, later, reexported, returned, run, tools } from "dep-run";

// Every call hands `count` to a package function that passes it on to a
// function value no analyzed source defines: SC9012 at each argument.
export function Panel() {
  const [count] = createSignal(1);
  run(count);
  aliased(count);
  destructured(count);
  returned(count);
  chained(count);
  later(count);
  reexported(count);
  tools.run(count);
  // Not a reactive source: nothing to report.
  run(() => 1);
  return <p>{count()}</p>;
}
