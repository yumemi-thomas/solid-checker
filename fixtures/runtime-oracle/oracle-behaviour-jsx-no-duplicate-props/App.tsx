// SC8003 jsx-no-duplicate-props, observed behaviourally. A textContent attribute and
// JSX children both write the element's content, so one of them is overwritten and
// the DOM shows only one source. The control uses children alone.
import { createSignal } from "solid-js";

export function Violation() {
  const [label] = createSignal("l");
  const [body] = createSignal("b");
  return (
    <section>
      <div data-probe="violation-content" textContent={label()}>{body()}</div>
    </section>
  );
}

export function Control() {
  const [body] = createSignal("b");
  return (
    <section>
      <div data-probe="control-content">{body()}</div>
    </section>
  );
}
