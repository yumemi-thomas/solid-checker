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
