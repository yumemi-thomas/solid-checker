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

// innerHTML beside children: innerHTML shows on first render and still after the
// children update, so the children never show (checked in Chrome on rc.13).
export function HtmlViolation() {
  const [markup] = createSignal("<i>h</i>");
  const [body, setBody] = createSignal("b");
  return (
    <section>
      <button data-probe="html-write" onClick={() => setBody("c")}>write</button>
      <div data-probe="html-content" innerHTML={markup()}>{body()}</div>
    </section>
  );
}
