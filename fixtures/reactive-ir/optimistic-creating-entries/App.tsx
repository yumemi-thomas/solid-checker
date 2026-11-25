import { createMemo, createOptimistic, flush } from "solid-js";

// ADR 1216: a write that throws on its first run is proven by one entry on the creating pass. A named helper's direct
// straight-line call, a memo's compute (it runs during `createMemo`) and `flush(fn)` (it runs `fn` before returning)
// carry the write there, whatever other uses the helper has. Each is OPTIMISTIC_WRITE_BEFORE_LOAD on rc.21.

// Violations.
export function HelperAlsoHanded() {
  const [value, setValue] = createOptimistic(async () => 1);
  const write = () => {
    setValue(2);
  };
  write();
  const kept = new Map<string, () => void>();
  kept.set("k", write);
  return <button onClick={() => kept.get("k")?.()}>{value()}</button>;
}

export function MemoCompute() {
  const [value, setValue] = createOptimistic(async () => 1);
  const memo = createMemo(() => {
    setValue(2);
    return 1;
  });
  return <p>{value()}{memo()}</p>;
}

export function FlushInHelper() {
  const [value, setValue] = createOptimistic(async () => 1);
  const write = () => {
    flush(() => {
      setValue(2);
    });
  };
  write();
  return <p>{value()}</p>;
}

export function SharedByMemoAndHandler() {
  const [value, setValue] = createOptimistic(async () => 1);
  const write = () => {
    setValue(2);
  };
  const memo = createMemo(() => {
    write();
    return 1;
  });
  return <button onClick={() => write()}>{value()}{memo()}</button>;
}

// Uncertifiable: no entry is proven to run on the creating pass.
export function HandlerOnly() {
  const [value, setValue] = createOptimistic(async () => 1);
  const write = () => {
    setValue(2);
  };
  return <button onClick={() => write()}>{value()}</button>;
}

// A call under a branch is no proven run (and the helper is also handed on, so it is no in-place layer either).
export function UnderBranch(props: { go?: boolean }) {
  const [value, setValue] = createOptimistic(async () => 1);
  const write = () => {
    setValue(2);
  };
  if (props.go) write();
  const kept = new Map<string, () => void>();
  kept.set("k", write);
  return <button onClick={() => kept.get("k")?.()}>{value()}</button>;
}

export function TimerOnly() {
  const [value, setValue] = createOptimistic(async () => 1);
  const write = () => {
    setValue(2);
  };
  setTimeout(write, 0);
  return <p>{value()}</p>;
}

// A call written in JSX runs when the compiled code reads it: this walk takes no JSX position (the helper is also handed on).
export function InJsx() {
  const [value, setValue] = createOptimistic(async () => 1);
  const write = () => {
    setValue(2);
    return "x";
  };
  const kept = new Map<string, () => string>();
  kept.set("k", write);
  return <p>{value()}<i>{write()}</i></p>;
}

export function LazyMemo() {
  const [value, setValue] = createOptimistic(async () => 1);
  const memo = createMemo(
    () => {
      setValue(2);
      return 1;
    },
    { lazy: true },
  );
  return <button onClick={() => memo()}>{value()}</button>;
}

export function AfterAwait() {
  const [value, setValue] = createOptimistic(async () => 1);
  const write = async () => {
    await Promise.resolve();
    setValue(2);
  };
  void write();
  return <p>{value()}</p>;
}
