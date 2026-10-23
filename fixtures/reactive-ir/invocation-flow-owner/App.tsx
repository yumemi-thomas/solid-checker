import { createSignal, createEffect, onCleanup, runWithOwner, getOwner, createRoot } from "solid-js";

import { imported } from "./lib";

function handlers(cb: () => void) {
  return { onClick: () => cb() };
}

// Positive: the callback runs from the click handler `handlers` returns,
// spread onto the button.
export function Spread() {
  const [count] = createSignal(0);
  const h = handlers(() => {
    createEffect(() => count(), () => undefined);
    onCleanup(() => undefined);
  });
  return <button {...h}>a</button>;
}

// Positive: the returned handler is given as an attribute.
export function Attribute() {
  const h = handlers(() => {
    onCleanup(() => undefined);
  });
  return <button onClick={h.onClick}>b</button>;
}

// Positive: the helper is imported.
export function Imported() {
  const h = imported(() => {
    onCleanup(() => undefined);
  });
  return <button {...h}>c</button>;
}

function owned(cb: () => void) {
  const owner = getOwner();
  return { onClick: () => runWithOwner(owner, cb) };
}

// Negative: the handler runs the callback under the component's owner.
export function UnderOwner() {
  const h = owned(() => {
    onCleanup(() => undefined);
  });
  return <button {...h}>d</button>;
}

function rooted(cb: () => void) {
  return { onClick: () => createRoot(() => cb()) };
}

// Negative: the handler runs the callback in a fresh root.
export function UnderRoot() {
  const h = rooted(() => {
    onCleanup(() => undefined);
  });
  return <button {...h}>e</button>;
}

// Mixed (ADR 0363): a click-run violation; the handler is also called in
// the component body, which has an owner.
export function AlsoInBody() {
  const h = handlers(() => {
    onCleanup(() => undefined);
  });
  h.onClick();
  return <button {...h}>f</button>;
}

function constructed(cb: () => void) {
  return {
    onClick: () => {
      class Later {
        value = cb();
      }
      createRoot(() => void new Later());
    },
  };
}

// Negative: the call is a class field initializer, run when `Later` is
// constructed inside `createRoot`, which supplies an owner.
export function InClassField() {
  const h = constructed(() => {
    onCleanup(() => undefined);
  });
  return <button {...h}>g</button>;
}

function defaulted(cb: () => void) {
  return {
    onClick: () => {
      function later(value = cb()) {
        return value;
      }
      createRoot(() => later());
    },
  };
}

// Negative: the call is a nested function's default parameter, run when
// `later` is called inside `createRoot`.
export function InDefault() {
  const h = defaulted(() => {
    onCleanup(() => undefined);
  });
  return <button {...h}>h</button>;
}
