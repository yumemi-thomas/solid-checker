import { onCleanup } from "solid-js";
import { render } from "@solidjs/web";
import { withLeaf } from "dep-leaf";

// The project names no primitive that opens a leaf scope, but `dep-leaf` (analyzed as source in this build) does, and runs the callback it is handed inside it: the
// `onCleanup` below is under a leaf owner (CLEANUP_IN_FORBIDDEN_SCOPE). The program census does not answer for it.

function Plain() {
  withLeaf(() => {
    onCleanup(() => {});
  });
  return <p>x</p>;
}

render(() => <Plain />, document.body);
