import { createMemo, createSignal, createStore, createRoot, runWithOwner, getOwner, untrack, Loading } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

export function DisposedByTimer() {
  const [value] = createSignal(1);
  createRoot((dispose) => {
    const data = createMemo(async () => {
      await pause();
      console.log("disposed read", value());
      return 0;
    });
    createMemo(() => data());
    setTimeout(dispose, 5);
  });
  return <span>disposed</span>;
}

export function DisposedInsideComputation() {
  const [value] = createSignal(1);
  createRoot((dispose) => {
    const data = createMemo(async () => {
      await pause();
      dispose();
      console.log("disposed read", value());
      return 0;
    });
    createMemo(() => data());
  });
  return <span>disposed</span>;
}

export function RootControl() {
  const [value] = createSignal(1);
  createRoot(() => {
    const data = createMemo(async () => {
      await pause();
      console.log("live read", value());
      return 0;
    });
    createMemo(() => data());
  });
  return <span>live</span>;
}
