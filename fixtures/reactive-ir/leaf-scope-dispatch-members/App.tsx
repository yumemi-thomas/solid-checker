import { onSettled } from "solid-js";

type Refs = { form?: HTMLFormElement; el?: HTMLDivElement; parent?: HTMLDivElement };

// ---- Open (ADR 0705): each member dispatches an event on its caller's stack
// (measured in Chrome: `formdata`, `error`, `blur` of a focused element,
// `beforeunload`), so a listener it reaches may call onCleanup in this leaf.

export function Submit(props: Refs) {
  onSettled(() => {
    props.form?.submit();
  });
  return null;
}
export function Report() {
  onSettled(() => {
    reportError(new Error("x"));
  });
  return null;
}
export function Replace(props: Refs) {
  onSettled(() => {
    props.el?.replaceWith(document.createElement("i"));
    props.parent?.replaceChildren();
  });
  return null;
}
export function Navigate() {
  onSettled(() => {
    location.reload();
  });
  return null;
}

// ---- Clean: a project method spelled like one is not the `lib.dom`
// declaration, and an inert member stays clean.
const form = { submit() {} };
export function ProjectSubmit(props: Refs) {
  onSettled(() => {
    form.submit();
    props.el?.getBoundingClientRect();
  });
  return null;
}
