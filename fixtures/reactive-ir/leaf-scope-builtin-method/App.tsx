import { onSettled } from "solid-js";

declare const dialog: HTMLDialogElement;
declare const maybeDialog: HTMLDialogElement | undefined;
declare const renderer: { setSize(): void };

// Negative: `dialog.focus()` and `getBoundingClientRect()` take no argument
// and are standard-library methods, which create no primitive and register
// no cleanup in the leaf scope.
export function BuiltinMethods() {
  onSettled(() => {
    dialog.focus();
    const rect = document.body.getBoundingClientRect();
    void rect;
  });
  return <div />;
}

// Known gap: Type Facts states no resolved call for an optional call, so the
// same built-in through `?.` cannot be certified yet.
export function OptionalBuiltin() {
  onSettled(() => {
    maybeDialog?.focus();
  });
  return <div />;
}

// Control: a method of a declared object has no body here and is no
// built-in, so the leaf scope still cannot be certified.
export function UnknownMethod() {
  onSettled(() => {
    renderer.setSize();
  });
  return <div />;
}
