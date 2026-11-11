import { flush } from "solid-js";
import { runPlain } from "fz-act";

// Nothing in the project or the dependency makes an action: the generator is a plain function a handler steps, where `flush` is
// legal. No row.
const steps = runPlain(function* () {
  flush();
  yield 1;
});

export function App() {
  return (
    <button
      onClick={() => {
        for (const _ of steps()) void _;
      }}
    >
      go
    </button>
  );
}
