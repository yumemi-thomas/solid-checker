import { onCleanup, onSettled } from "solid-js";

// The DOM type does not close the event-listener graph or prove that a
// deferred mutable reference still holds the native implementation.
export function OptionalDomFocus(props: { dialog?: HTMLDivElement }) {
  onSettled(() => {
    props.dialog?.focus();
  });
  return <div />;
}

// Registering a listener is not executing it. Focusing can execute it
// synchronously; its registration alone must not discharge the focus call.
export function FocusWithListener(props: { dialog?: HTMLDivElement }) {
  document.body.addEventListener("focus", () => onCleanup(() => {}), true);
  onSettled(() => {
    props.dialog?.focus();
  });
  return <div />;
}

export function UnknownFocus(props: { target: { focus(): void } }) {
  onSettled(() => {
    props.target.focus();
  });
  return <div />;
}

class InertTarget {
  focus() {}
}

class CleanupTarget {
  focus() {
    onCleanup(() => {});
  }
}

// The same spelling is clean only because its exact local body is inert.
export function ExactInertFocus() {
  const target = new InertTarget();
  onSettled(() => {
    target.focus();
  });
  return <div />;
}

// A same-name local method can register cleanup in the leaf scope.
export function ExactForbiddenFocus() {
  const target = new CleanupTarget();
  onSettled(() => {
    target.focus();
  });
  return <div />;
}
