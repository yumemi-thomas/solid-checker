// SC1005 uncalled-accessor, observed behaviourally. Joining an accessor into a
// string renders the accessor's own source, and the write never reaches the text.
// The control calls the accessor, so the text follows the write. Each reflect button
// sets the value its observation declares, and each probe is a correct binding of the
// same signal, so a write that changes nothing is a no-op rather than a lost write.
import { createSignal } from "solid-js";

export function Violation() {
  const [count, setCount] = createSignal(0);
  return (
    <section>
      <button data-probe="violation-write" onClick={() => setCount(7)}>write</button>
      <button data-probe="violation-reflect" onClick={() => setCount(101)}>reflect</button>
      <p data-probe="violation-label">{"Count: " + count}</p><i data-probe="violation-probe">{count()}</i>
    </section>
  );
}

export function Control() {
  const [count, setCount] = createSignal(0);
  return (
    <section>
      <button data-probe="control-write" onClick={() => setCount(7)}>write</button>
      <button data-probe="control-reflect" onClick={() => setCount(102)}>reflect</button>
      <p data-probe="control-label">{"Count: " + count()}</p><i data-probe="control-probe">{count()}</i>
    </section>
  );
}
