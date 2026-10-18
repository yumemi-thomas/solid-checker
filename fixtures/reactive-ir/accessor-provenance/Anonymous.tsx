import { createSignal } from "solid-js";

function invoke<T>(callback: () => T): T { return callback(); }
declare function unknownConsumer(callback: () => unknown): void;

export function InvokedInline() {
  const [read] = createSignal(1);
  const value = invoke(() => read());
  return <p>{value}</p>;
}

export function CapturedOnly() {
  const [read] = createSignal(1);
  function factory() { return () => read(); }
  const callback = factory();
  void callback;
  return <p />;
}

export function InvokedReturned() {
  const [read] = createSignal(1);
  function factory() { return () => read(); }
  const callback = factory();
  const value = callback();
  return <p>{value}</p>;
}

export function UnknownConsumer() {
  const [read] = createSignal(1);
  unknownConsumer(() => read());
  return <p />;
}

function unknownFactory() {
  const [read] = createSignal(1);
  unknownConsumer(() => read());
}
export function UnknownFactoryCall() {
  unknownFactory();
  return <p />;
}

export function execute(input: { callback: () => number }) {
  return invoke(() => input.callback());
}
declare const opaqueCallback: () => number;
export function CapturedParameterDispatch() {
  const value = execute({ callback: opaqueCallback });
  return <p>{value}</p>;
}
