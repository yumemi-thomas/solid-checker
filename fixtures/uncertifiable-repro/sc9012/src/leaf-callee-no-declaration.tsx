// Leaf callbacks calling something that names no readable declaration:
// solid-groove AppErrorFallback.tsx:40 (`(props.report ?? report)(...)`),
// app-game solid-dockview/dom-attrs.tsx:17 (a local holding a function through
// an assertion), solid-three/core/index.tsx:122 (destructured from `config || {}`).
import { createSignal, createTrackedEffect, onSettled } from "solid-js";

function report(error: unknown) {
  console.error(error);
}

export function Fallback(props: { error: unknown; report?: (error: unknown) => void }) {
  onSettled(() => {
    (props.report ?? report)(props.error);
  });
  return <p>failed</p>;
}

export function Logger() {
  const [message, setMessage] = createSignal("");
  const write = report as (message: string) => void;
  createTrackedEffect(() => {
    write(message());
  });
  return <input onInput={(event) => setMessage(event.currentTarget.value)} />;
}

export function createCanvasRoot(config?: { onCreated?: (size: number) => void }) {
  const { onCreated } = config || {};
  const [size, setSize] = createSignal(0);
  createTrackedEffect(() => {
    onCreated?.(size());
  });
  return setSize;
}
