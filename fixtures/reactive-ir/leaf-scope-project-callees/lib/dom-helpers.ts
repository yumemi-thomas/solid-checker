import { onCleanup } from "solid-js";

// DOM work only: sets an attribute, reads layout, scrolls.
export function domWork(el: HTMLElement) {
  el.dataset.x = "1";
  el.getBoundingClientRect();
  el.scrollIntoView();
}

export const arrowWork = (el: HTMLElement) => {
  el.title = "t";
};

// Registers a cleanup: runs in the leaf scope of whoever calls it.
export function leaky() {
  onCleanup(() => {});
}

// Reaches `leaky` through a second project helper.
export function outer() {
  inner();
}
function inner() {
  leaky();
}

// A refused synchronous dispatcher (ADR 0329): the click can run a listener
// that registers a cleanup.
export function clicker(el: HTMLElement) {
  el.click();
}
