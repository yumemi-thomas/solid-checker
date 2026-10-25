import { onCleanup, onSettled } from "solid-js";

type Refs = { el?: HTMLDivElement; canvas?: HTMLCanvasElement; input?: HTMLInputElement };

// ---- Clean: an optional call is read like its non-optional form (ADR 0318,
// ADR 0397): the exact `lib.dom` declaration decides, and only a member that
// dispatches an event synchronously or creates a WebGL context stays open.

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

// ---- Clean: optional calls of members that dispatch no event (ADR 0397).

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
export function OptionalSelect(props: Refs) {
  onSettled(() => {
    props.input?.select();
  });
  return <div />;
}

// Clean: a `let` ref assigned later, typed as possibly unset.
export function LetRefScroll() {
  let el: HTMLDivElement | undefined;
  const assign = (node: HTMLDivElement) => {
    el = node;
  };
  onSettled(() => {
    el?.scrollIntoView({ block: "nearest" });
    el!.scrollTo(0, 0);
  });
  return <div ref={assign} />;
}
export function LetRefObserver() {
  let observer: ResizeObserver | undefined;
  let target: HTMLDivElement | undefined;
  const assign = (node: HTMLDivElement) => {
    target = node;
    observer = new ResizeObserver(() => {});
  };
  onSettled(() => {
    if (target) observer?.observe(target);
  });
  return <div ref={assign} />;
}

// ---- Uncertifiable: optional calls of members that dispatch an event
// synchronously (ADR 0329) or create a WebGL context (ADR 0318).

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
export function OptionalGetContext(props: Refs) {
  onSettled(() => {
    props.canvas?.getContext("webgl2");
  });
  return <div />;
}
export function LetRefFocus() {
  let dialog: HTMLDialogElement | undefined;
  const assign = (node: HTMLDialogElement) => {
    dialog = node;
  };
  onSettled(() => {
    dialog?.focus();
  });
  return <dialog ref={assign} />;
}
export function LetRefShowModal() {
  let dialog: HTMLDialogElement | undefined;
  const assign = (node: HTMLDialogElement) => {
    dialog = node;
  };
  onSettled(() => {
    dialog?.showModal();
  });
  return <dialog ref={assign} />;
}

// ---- The non-optional form of a dispatching member is refused alike
// (ADR 0329).

export function DirectFocus(props: { el: HTMLDivElement }) {
  onSettled(() => {
    props.el.focus();
  });
  return <div />;
}
