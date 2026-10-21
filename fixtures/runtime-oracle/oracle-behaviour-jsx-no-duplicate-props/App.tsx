// SC8003 jsx-no-duplicate-props, observed behaviourally. A textContent attribute and
// JSX children both write the element's content, so one of them is overwritten and
// the DOM shows only one source. Each write is a fresh value (a counter), so a write
// that reaches the element is a real change; a repeated constant would read as lost.
// The two controls have one content source each.
import { createSignal } from "solid-js";

let tick = 0;
const fresh = (prefix: string) => `${prefix}${++tick}`;

export function Violation() {
  const [label, setLabel] = createSignal("l");
  const [body] = createSignal("b");
  return (
    <section>
      <button data-probe="violation-write" onClick={() => setLabel(fresh("l"))}>write</button>
      <div data-probe="violation-content" textContent={label()}>{body()}</div>
    </section>
  );
}

// innerHTML beside children: innerHTML shows on first render and still after the
// children update, so the children never show (checked in Chrome on rc.13).
export function HtmlViolation() {
  const [markup, setMarkup] = createSignal("<i>h</i>");
  const [body, setBody] = createSignal("b");
  return (
    <section>
      <button data-probe="html-write-markup" onClick={() => setMarkup(`<i>${fresh("h")}</i>`)}>markup</button>
      <button data-probe="html-write-body" onClick={() => setBody(fresh("c"))}>body</button>
      <div data-probe="html-content" innerHTML={markup()}>{body()}</div>
    </section>
  );
}

export function TextControl() {
  const [label, setLabel] = createSignal("l");
  return (
    <section>
      <button data-probe="text-control-write" onClick={() => setLabel(fresh("l"))}>write</button>
      <div data-probe="text-control-content" textContent={label()} />
    </section>
  );
}

export function Control() {
  const [body, setBody] = createSignal("b");
  return (
    <section>
      <button data-probe="control-write" onClick={() => setBody(fresh("b"))}>write</button>
      <div data-probe="control-content">{body()}</div>
    </section>
  );
}
