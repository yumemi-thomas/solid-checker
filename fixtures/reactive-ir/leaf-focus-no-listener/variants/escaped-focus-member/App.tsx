import { onCleanup, onSettled } from "solid-js";

// A class member written `f\u006fcus` is the member `focus`: a custom element's override runs on the stack of the leaf callback's `focus()`, though the types name the element's.
class Field extends HTMLElement {
  f\u006fcus() {
    onCleanup(() => {});
  }
}
customElements.define("x-field", Field);

export function Leaf() {
  let field: HTMLElement | undefined;
  onSettled(() => {
    field?.focus();
  });
  return <div ref={(el) => (field = el.appendChild(document.createElement("x-field")))} />;
}
