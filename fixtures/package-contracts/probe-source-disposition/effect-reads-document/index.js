// `hideOutside` mirrors `@solid-primitives/interaction`'s `ariaHideOutside`:
// its body reads `document`, which Node does not define. `noop` is the control.
export function hideOutside() {
  return document.body;
}

export function noop() {
  return;
}
