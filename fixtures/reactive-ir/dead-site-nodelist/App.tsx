import { action, createSignal, onFlush } from "solid-js";

const act = action(function* () {
  yield 1;
});

// ---- Never run (ADR 1033): the NodeList holds no node, so its `forEach` never calls the callback; U, never a violation ----

export function FreshChildNodes() {
  const [, setW] = createSignal(0);
  const tf = () => {
    setW(1);
  };
  (document.createElement("i").childNodes as NodeList).forEach(tf);
  return <p />;
}
export function FreshQueryAction() {
  document.createElement("div").querySelectorAll("span").forEach(() => {
    void act();
  });
  return <p />;
}
export function FreshQueryFlush() {
  document.createElement("p").querySelectorAll("b").forEach(() => {
    onFlush(() => {});
  });
  return <p />;
}

// ---- May run: today's reading stays (a violation) ----

// The document may hold any element: what it holds is not read.
export function DocumentQuery() {
  const [, setW] = createSignal(0);
  document.querySelectorAll("sc-no-such-tag").forEach(() => {
    setW(3);
  });
  return <p />;
}
export function BodyChildNodes() {
  const [, setW] = createSignal(0);
  document.body.childNodes.forEach(() => {
    setW(4);
  });
  return <p />;
}
// A name with a `-` may be a custom element, whose constructor may append children.
export function CustomElementChildNodes() {
  const [, setW] = createSignal(0);
  document.createElement("x-card").childNodes.forEach(() => {
    setW(5);
  });
  return <p />;
}
// A node held by a name may have been given children.
export function NamedNode() {
  const [, setW] = createSignal(0);
  const node = document.createElement("i");
  node.append(document.createElement("b"));
  node.childNodes.forEach(() => {
    setW(6);
  });
  return <p />;
}
// A local `document` is not the library's.
export function ShadowedDocument() {
  const [, setW] = createSignal(0);
  const document = window.document;
  document.createElement("i").childNodes.forEach(() => {
    setW(7);
  });
  return <p />;
}
// A fragment is not read (its `createDocumentFragment()` call has no resolved declaration here): a known false positive, docs/open-work.md.
export function FreshFragmentChildNodes() {
  const [, setW] = createSignal(0);
  document.createDocumentFragment().childNodes.forEach(() => {
    setW(2);
  });
  return <p />;
}
export function FreshFragmentQueryFlush() {
  document.createDocumentFragment().querySelectorAll("p").forEach(() => {
    onFlush(() => {});
  });
  return <p />;
}
