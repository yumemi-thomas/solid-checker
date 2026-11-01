import { createMemo, createSignal } from "solid-js";

export function ConditionalReadInBody() {
  const [count] = createSignal(0);
  const read = Math.random() < 2 ? count : count;
  read();
  return <div />;
}

export function IdentityWriteInMemo() {
  const [, setCount] = createSignal(0);
  const write = ((value: typeof setCount) => value)(setCount);
  const memo = createMemo(() => {
    write(1);
    return 1;
  });
  return <div>{memo()}</div>;
}

export function ConditionalReadInMemo() {
  const [count] = createSignal(0);
  const read = Math.random() < 2 ? count : count;
  const memo = createMemo(() => read());
  return <div>{memo()}</div>;
}

export function IdentityWriteInHandler() {
  const [, setCount] = createSignal(0);
  const write = ((value: typeof setCount) => value)(setCount);
  return <button onClick={() => write(1)}>go</button>;
}

export function ConditionalOfTwoSignals() {
  const [count] = createSignal(0);
  const [other] = createSignal(0);
  const read = Math.random() < 2 ? count : other;
  read();
  return <div />;
}

export function IdentityWriteInBody() {
  const [, setCount] = createSignal(0);
  const write = ((value: typeof setCount) => value)(setCount);
  write(1);
  return <div />;
}


export function IdentityReadInBody() {
  const [count] = createSignal(0);
  const read = ((value: typeof count) => value)(count);
  read();
  return <div />;
}
