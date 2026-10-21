// SC1005 uncalled-accessor, observed behaviourally. Joining an accessor into a
// string renders the accessor's own source, and the write never reaches the text.
// The control calls the accessor, so the text follows the write. The reflect buttons
// write a fresh number on every click (counted from 100, above the constant writes),
// so a write that reaches the signal is a real change.
import { createSignal } from "solid-js";

let tick = 100;

export function Violation() {
  const [count, setCount] = createSignal(0);
  return (
    <section>
      <button data-probe="violation-write" onClick={() => setCount(7)}>write</button>
      <button data-probe="violation-reflect" onClick={() => setCount(++tick)}>reflect</button>
      <p data-probe="violation-label">{"Count: " + count}</p>
    </section>
  );
}

export function Control() {
  const [count, setCount] = createSignal(0);
  return (
    <section>
      <button data-probe="control-write" onClick={() => setCount(7)}>write</button>
      <button data-probe="control-reflect" onClick={() => setCount(++tick)}>reflect</button>
      <p data-probe="control-label">{"Count: " + count()}</p>
    </section>
  );
}
