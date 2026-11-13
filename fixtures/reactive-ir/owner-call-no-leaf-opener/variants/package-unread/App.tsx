import { onCleanup } from "solid-js";
import { render } from "@solidjs/web";
import { withLeaf } from "dep-leaf";

// The project names no primitive that opens a leaf scope, but `dep-leaf` does, and runs the callback it is handed inside it: the `onCleanup` below is under a leaf owner
// (CLEANUP_IN_FORBIDDEN_SCOPE). The standard build does not read the package's source, so nothing here shows it: a package the build does not read may run the function, and
// the program census does not answer for it (ADR 0967).

function Plain() {
  withLeaf(() => {
    onCleanup(() => {});
  });
  return <p>x</p>;
}

render(() => <Plain />, document.body);
