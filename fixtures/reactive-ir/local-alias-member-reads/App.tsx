import { action, createMemo, createSignal } from "solid-js";

// ADR 0481: `thing.key`, where `thing` holds an object-literal arrow's fresh
// result and is only ever read that way, is the `const` the key names.

// Violation: a signal write in a memo through the member (rc.17 throws).
export function SetterMemberInMemo() {
  const [count, setCount] = createSignal(0);
  const useCounter = () => ({ set: setCount });
  const counter = useCounter();
  const m = createMemo(() => {
    counter.set(1);
    return 1;
  });
  return <div>{count()}{m()}</div>;
}

// Clean: the same write in a handler lands.
export function SetterMemberInHandler() {
  const [count, setCount] = createSignal(0);
  const useCounter = () => ({ set: setCount });
  const counter = useCounter();
  return <button onClick={() => counter.set(1)}>{count()}</button>;
}

// Violation: an action called in a memo through the member.
export function ActionMemberInMemo() {
  const [count, setCount] = createSignal(0);
  const save = action(function* () {
    setCount(1);
  });
  const useSaver = () => ({ save });
  const saver = useSaver();
  const m = createMemo(() => {
    saver.save();
    return 1;
  });
  return <div>{count()}{m()}</div>;
}

// Not an alias: the holder is handed out, so its property may be replaced.
export function HolderPassed() {
  const [count, setCount] = createSignal(0);
  const useCounter = () => ({ set: setCount });
  const counter = useCounter();
  console.log(counter);
  const m = createMemo(() => {
    counter.set(1);
    return 1;
  });
  return <div>{count()}{m()}</div>;
}
