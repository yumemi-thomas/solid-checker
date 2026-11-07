import { createMemo } from "solid-js";
import { render } from "@solidjs/web";

// ADR 0838 (R2h): a `try` around a pending read swallows its throw wherever
// the carrier that runs the function stands (Chrome rc.21: CAUGHT_REACTIVE_READ
// only, no boundary warning). The build before looked for the `try` at the
// carrier (`[1].forEach(handed)`) and proved SC5003 at the read.
function CaughtHandedRead() {
  const pending = createMemo(async () => 1);
  return (
    <p>
      {(() => {
        const handed = () => {
          try {
            pending();
          } catch {}
        };
        [1].forEach(handed);
        return 1;
      })()}
    </p>
  );
}

// The same carrier through `.call`.
function CaughtCalledRead() {
  const pending = createMemo(async () => 1);
  return (
    <p>
      {(() => {
        const handed = () => {
          try {
            pending();
          } catch {}
        };
        handed.call(undefined);
        return 1;
      })()}
    </p>
  );
}

// The twin with no `try`: the throw reaches the render effect with no
// `Loading` above it, and the boundary claim stands.
function HandedRead() {
  const pending = createMemo(async () => 1);
  return (
    <p>
      {(() => {
        const handed = () => {
          pending();
        };
        [1].forEach(handed);
        return 1;
      })()}
    </p>
  );
}

render(() => <CaughtHandedRead />, document.body);
render(() => <CaughtCalledRead />, document.body);
render(() => <HandedRead />, document.body);
