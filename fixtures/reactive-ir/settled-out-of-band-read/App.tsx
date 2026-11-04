import { createMemo, onSettled } from "solid-js";
import * as S from "solid-js";

// Violation: `onSettled` under the component's owner is a leaf, through a helper or directly.
export function OwnedHelper() {
  const value = createMemo(async () => 1);
  const wrapUse = () => {
    value();
  };
  onSettled(() => {
    wrapUse();
  });
  return <div />;
}

export function OwnedDirect() {
  const value = createMemo(async () => 1);
  onSettled(() => {
    value();
  });
  return <div />;
}

// Uncertifiable, not a violation: a handler's `onSettled` queues a plain callback, no leaf. Through a helper the read
// answers as the same read written in the callback does.
export function HandlerHelper() {
  const value = createMemo(async () => 1);
  const wrapUse = () => {
    value();
  };
  return <button onClick={() => onSettled(() => wrapUse())}>x</button>;
}

export function HandlerDirect() {
  const value = createMemo(async () => 1);
  return <button onClick={() => onSettled(() => value())}>x</button>;
}

// Uncertifiable: a closure run inside the callback, and a callback handed by name.
export function HandlerClosure() {
  const value = createMemo(async () => 1);
  return (
    <button
      onClick={() => {
        onSettled(() => {
          const run = () => {
            value();
          };
          run();
        });
      }}
    >
      x
    </button>
  );
}

export function HandlerNamed() {
  const value = createMemo(async () => 1);
  const settled = () => {
    value();
  };
  return <button onClick={() => onSettled(settled)}>x</button>;
}

// Uncertifiable: the namespace form is the same primitive.
export function HandlerNamespace() {
  const value = S.createMemo(async () => 1);
  const wrapUse = () => {
    value();
  };
  return <button onClick={() => S.onSettled(() => wrapUse())}>x</button>;
}
