import { createSignal, createStore } from "solid-js";

// Proved: after `.then`, the store setter runs only its updater, which writes a literal under a static key of the draft.
export function ThenDraft() {
  const [s1, setS1] = createStore({ v: 0 });
  Promise.resolve().then(() => {
    setS1((draft) => { draft.v = 7; });
  });
  return <div>{s1.v}</div>;
}

// Proved: after an `await`, the same write through the store root inside the setter.
export function AwaitRoot() {
  const [s2, setS2] = createStore({ v: 0 });
  const run = async () => {
    await Promise.resolve();
    setS2(() => { s2.v = 7; });
  };
  run();
  return <div>{s2.v}</div>;
}

// Open: the updater reads a signal after the await.
export function ReadsSignal() {
  const [count] = createSignal(1);
  const [s3, setS3] = createStore({ v: 0 });
  Promise.resolve().then(() => {
    setS3((draft) => { draft.v = count(); });
  });
  return <div>{s3.v}</div>;
}

// Open: the updater copies a member it reads.
export function ReadsMember() {
  const [s4, setS4] = createStore({ v: 0, w: 1 });
  Promise.resolve().then(() => {
    setS4((draft) => { draft.v = draft.w; });
  });
  return <div>{s4.v}</div>;
}

// Open: the derived form, whose store a function makes.
export function Derived() {
  const [s5, setS5] = createStore((draft: { v: number }) => { draft.v = 1; }, { v: 0 });
  Promise.resolve().then(() => {
    setS5((draft) => { draft.v = 7; });
  });
  return <div>{s5.v}</div>;
}
