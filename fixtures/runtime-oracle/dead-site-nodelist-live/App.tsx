import { createSignal } from "solid-js";

// ---- Runs: the NodeList holds a node, so the write runs in the component body (Chrome: REACTIVE_WRITE_IN_OWNED_SCOPE) ----

export function BodyChildNodes() {
  const [, setW] = createSignal(0);
  document.body.childNodes.forEach(() => {
    setW(4);
  });
  return <p />;
}
export function NamedNode() {
  const [, setW] = createSignal(0);
  const node = document.createElement("i");
  node.append(document.createElement("b"));
  node.childNodes.forEach(() => {
    setW(6);
  });
  return <p />;
}
