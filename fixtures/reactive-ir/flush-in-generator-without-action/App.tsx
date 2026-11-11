import { flush } from "solid-js";

// No action is named anywhere in the program: no callback is stepped as an action's body, so a flush in a generator
// is no FLUSH_IN_ACTION site (ADR 0961's premise).
function* steps() {
  flush();
  yield 1;
}

export function App() {
  return (
    <button
      onClick={() => {
        for (const step of steps()) void step;
      }}
    >
      go
    </button>
  );
}
