import { createTrackedEffect, onSettled } from "solid-js";

// Nothing in this program listens for what a focus move delivers (`focus`, `blur`, `focusin`, `focusout`, `change`, the composition
// and input events of a commit) or stands in for `focus`, so an element's `focus()` runs no project code on the stack of a leaf
// owner's callback: ADR 0329's refusal is lifted for that one member (`focus_delivery`).

// Clean: the textbook shape (a `let` ref set by a ref callback).
export function LetRef() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => (input = el)} />;
}

// Clean: the non-optional form.
export function Direct() {
  let box!: HTMLDivElement;
  onSettled(() => {
    box.focus();
  });
  return <div ref={(el) => (box = el)} />;
}

// Clean: an option bag of data is inert.
export function OptionBag() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus({ preventScroll: true });
  });
  return <input ref={(el) => (input = el)} />;
}

// Clean: the other leaf owner.
export function Tracked() {
  let box: HTMLDivElement | undefined;
  createTrackedEffect(() => {
    box?.focus();
  });
  return <div ref={(el) => (box = el)} />;
}

// Clean: a helper the callback calls is walked, and its `focus()` is the same call.
function focusIt(el: HTMLElement | undefined) {
  el?.focus();
}
export function ViaHelper() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    focusIt(input);
  });
  return <input ref={(el) => (input = el)} />;
}

// ---- Open: the proof is for an element's `focus()` alone.

// `blur()` delivers `blur`, `focusout` and `change` to the element losing focus: not this proof's.
export function Blur() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.blur();
  });
  return <input ref={(el) => (input = el)} />;
}

// `click()`, a dialog's `showModal()`: other deliveries.
export function Click() {
  let box: HTMLDivElement | undefined;
  onSettled(() => {
    box?.click();
  });
  return <div ref={(el) => (box = el)} />;
}
export function ShowModal() {
  let dialog: HTMLDialogElement | undefined;
  onSettled(() => {
    dialog?.showModal();
  });
  return <dialog ref={(el) => (dialog = el)} />;
}

// The window's own `focus()`: the bare global, not an element's member.
export function BareFocus() {
  onSettled(() => {
    focus();
  });
  return <div />;
}

// The host reads the option bag's members while it converts the argument: a binding may hold an object with a getter, and a literal
// with an accessor has code in it.
export function OptionsBinding() {
  let input: HTMLInputElement | undefined;
  const options: FocusOptions = { preventScroll: true };
  onSettled(() => {
    input?.focus(options);
  });
  return <input ref={(el) => (input = el)} />;
}
export function OptionsGetter() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus({
      get preventScroll() {
        return true;
      },
    });
  });
  return <input ref={(el) => (input = el)} />;
}
