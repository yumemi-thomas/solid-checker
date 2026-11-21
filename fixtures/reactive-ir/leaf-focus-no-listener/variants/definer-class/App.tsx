import { onCleanup, onSettled } from "solid-js";

// A custom element's own `focus` is the one `field.focus()` runs, though the types name the element's.
class Field extends HTMLElement {
  focus() {
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
