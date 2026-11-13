import { onCleanup } from "solid-js";
import { render } from "@solidjs/web";

// The program names no primitive that opens a leaf scope (`createTrackedEffect`, `onSettled`), so no leaf owner exists to forbid an `onCleanup`: the program census answers
// for SC3001's CLEANUP_IN_FORBIDDEN_SCOPE, for every call whose function no package the build does not read may run.

// A method of a local object, called directly. No owner edge enters a method, so the owner graph answers nothing for the call below; the census does.
const api = {
  run() {
    onCleanup(() => {});
  },
};

function Plain() {
  api.run();
  return <p>x</p>;
}

render(() => <Plain />, document.body);
