import { createSignal, flush, untrack } from "solid-js";

const [count] = createSignal(1);

// Handed to `map` as a plain function as well as returning JSX: whether its
// body runs as a component is not proven, and `flush(fn)` runs its callback
// where the body runs, so the read there is an obligation.
function Row() {
  flush(() => {
    count();
  });
  return <div />;
}

// Rendered by a tag only: a component body; the same read is a violation.
function Rendered() {
  flush(() => {
    count();
  });
  return <div />;
}

// `untrack(fn)` closes the window: silent either way.
function Quiet() {
  untrack(() => count());
  return <div />;
}

export function App() {
  return (
    <div>
      {[1, 2].map(Row)}
      <Rendered />
      <Quiet />
    </div>
  );
}
