// SC8003 jsx-no-duplicate-props, observed behaviourally. Two sources write one element's
// content and the DOM shows one of them, so the other source's write is lost. Each source
// has a button that sets the value its observation declares, and a probe bound to the same
// signal, so a lost write is a real change the DOM ignored. The one-source controls reflect
// every write. StaticText's textContent is never written, so no write reaches it: it is
// static-only, and the static text never shows.
import { createSignal } from "solid-js";

export function Violation() {
  const [label, setLabel] = createSignal("l");
  const [body, setBody] = createSignal("b");
  return (
    <section>
      <button data-probe="violation-write" onClick={() => setLabel("l-1")}>write</button>
      <button data-probe="violation-body" onClick={() => setBody("b-1")}>body</button>
      <div data-probe="violation-content" textContent={label()}>{body()}</div>
      <i data-probe="violation-label-probe">{label()}</i><i data-probe="violation-body-probe">{body()}</i>
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
      <button data-probe="html-write-markup" onClick={() => setMarkup("<i>h-1</i>")}>markup</button>
      <button data-probe="html-write-body" onClick={() => setBody("b-1")}>body</button>
      <div data-probe="html-content" innerHTML={markup()}>{body()}</div>
      <i data-probe="html-markup-probe">{markup()}</i><i data-probe="html-body-probe">{body()}</i>
    </section>
  );
}

// A children prop beside textContent: the textContent wins and the prop is lost.
export function ChildrenPropText() {
  const [t, setT] = createSignal("t");
  const [c, setC] = createSignal("c");
  return (
    <section>
      <button data-probe="children-text-write" onClick={() => setT("t-1")}>text</button>
      <button data-probe="children-prop-write" onClick={() => setC("c-1")}>children</button>
      <div data-probe="children-text-content" children={c()} textContent={t()} />
      <i data-probe="children-text-probe">{t()}</i><i data-probe="children-prop-probe">{c()}</i>
    </section>
  );
}

// innerHTML beside textContent: the innerHTML wins and the textContent is lost.
export function HtmlText() {
  const [t, setT] = createSignal("t");
  const [m, setM] = createSignal("<i>m</i>");
  return (
    <section>
      <button data-probe="html-text-write" onClick={() => setT("t-1")}>text</button>
      <button data-probe="html-text-markup-write" onClick={() => setM("<i>m-1</i>")}>markup</button>
      <div data-probe="html-text-content" innerHTML={m()} textContent={t()} />
      <i data-probe="html-text-text-probe">{t()}</i><i data-probe="html-text-markup-probe">{m()}</i>
    </section>
  );
}

// A children prop beside innerHTML: the innerHTML wins and the prop is lost.
export function ChildrenPropHtml() {
  const [m, setM] = createSignal("<i>m</i>");
  const [c, setC] = createSignal("c");
  return (
    <section>
      <button data-probe="children-html-markup-write" onClick={() => setM("<i>m-1</i>")}>markup</button>
      <button data-probe="children-html-children-write" onClick={() => setC("c-1")}>children</button>
      <div data-probe="children-html-content" children={c()} innerHTML={m()} />
      <i data-probe="children-html-markup-probe">{m()}</i><i data-probe="children-html-children-probe">{c()}</i>
    </section>
  );
}

// A static textContent beside children: the children win on first render; the static text is
// never shown and no write can reach it.
export function StaticText() {
  const [c] = createSignal("c");
  return (
    <section>
      <div data-probe="static-text-content" textContent="static">{c()}</div>
    </section>
  );
}

export function TextControl() {
  const [label, setLabel] = createSignal("l");
  return (
    <section>
      <button data-probe="text-control-write" onClick={() => setLabel("l-1")}>write</button>
      <div data-probe="text-control-content" textContent={label()} />
      <i data-probe="text-control-probe">{label()}</i>
    </section>
  );
}

export function Control() {
  const [body, setBody] = createSignal("b");
  return (
    <section>
      <button data-probe="control-write" onClick={() => setBody("b-1")}>write</button>
      <div data-probe="control-content">{body()}</div>
      <i data-probe="control-probe">{body()}</i>
    </section>
  );
}
