import { onCleanup, onSettled } from "solid-js";

type Refs = { el?: HTMLDivElement; canvas?: HTMLCanvasElement; input?: HTMLInputElement };

// ---- Clean: optional calls of audited script-free members (ADR 0318) are
// read like their non-optional form.

export function OptionalQueries(props: Refs) {
  onSettled(() => {
    props.el?.querySelector("a");
    props.el?.querySelectorAll("a");
    props.el?.getElementsByTagName("a");
    props.el?.getElementsByClassName("x");
    props.el?.getBoundingClientRect();
    props.el?.getClientRects();
    props.el?.contains(null);
    props.el?.classList.contains("x");
    props.el?.closest("section");
    props.el?.matches(".x");
    props.el?.getAttribute("id");
    props.el?.getAttributeNames();
    props.el?.hasAttribute("id");
    props.el?.hasAttributes();
    props.canvas?.toDataURL();
  });
  return <div />;
}

// Clean: registration runs nothing; the listener is a deferred argument even
// when it would register a cleanup.
export function OptionalListener(props: Refs) {
  onSettled(() => {
    props.el?.addEventListener("click", () => onCleanup(() => {}));
  });
  return <div />;
}

// Uncertifiable in both forms: the standard-library timing table audits no
// timing for `removeEventListener`'s callable argument (ADR 0210), whatever
// the call's form.
export function OptionalRemoveListener(props: Refs) {
  onSettled(() => {
    const listener = () => {};
    props.el?.removeEventListener("click", listener);
  });
  return <div />;
}
export function DirectRemoveListener(props: { el: HTMLDivElement }) {
  onSettled(() => {
    const listener = () => {};
    props.el.removeEventListener("click", listener);
  });
  return <div />;
}

// ---- Uncertifiable: optional calls of members that can run author code.

export function OptionalFocus(props: Refs) {
  onSettled(() => {
    props.el?.focus();
  });
  return <div />;
}
export function OptionalClick(props: Refs) {
  onSettled(() => {
    props.el?.click();
  });
  return <div />;
}
export function OptionalDispatch(props: Refs) {
  onSettled(() => {
    props.el?.dispatchEvent(new Event("x"));
  });
  return <div />;
}
export function OptionalSetAttribute(props: Refs) {
  onSettled(() => {
    props.el?.setAttribute("data-x", "1");
  });
  return <div />;
}
export function OptionalAppend(props: Refs) {
  onSettled(() => {
    props.el?.append("text");
  });
  return <div />;
}
export function OptionalRemove(props: Refs) {
  onSettled(() => {
    props.el?.remove();
  });
  return <div />;
}
export function OptionalScroll(props: Refs) {
  onSettled(() => {
    props.el?.scrollIntoView();
  });
  return <div />;
}
export function OptionalGetContext(props: Refs) {
  onSettled(() => {
    props.canvas?.getContext("webgl2");
  });
  return <div />;
}
export function OptionalSelect(props: Refs) {
  onSettled(() => {
    props.input?.select();
  });
  return <div />;
}

// ---- Unchanged: the non-optional form of a re-entrant member keeps the
// standard-library trust, which does not model dispatch.

export function DirectFocus(props: { el: HTMLDivElement }) {
  onSettled(() => {
    props.el.focus();
  });
  return <div />;
}
