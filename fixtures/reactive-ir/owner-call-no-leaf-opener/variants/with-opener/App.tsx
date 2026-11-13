import { createTrackedEffect, onCleanup } from "solid-js";
import { render } from "@solidjs/web";

// The same program with a leaf opener: the census no longer answers, and the method's `onCleanup` keeps its uncertifiable row, because no owner edge enters a method and a
// leaf owner may run it.

const api = {
  run() {
    onCleanup(() => {});
  },
};

function Plain() {
  api.run();
  return <p>x</p>;
}

function Opener() {
  createTrackedEffect(() => {});
  return <p>opener</p>;
}

render(
  () => (
    <p>
      <Plain />
      <Opener />
    </p>
  ),
  document.body,
);
