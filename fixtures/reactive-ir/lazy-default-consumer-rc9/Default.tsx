import { createSignal, lazy } from "solid-js";
import createLoop from "dep-loop";

const frame = () => {};

// app-game's `TestRapier2D` shape: the loaded `default` is a component
// declared in the loader, and its own body calls `start()`, which reads
// `running`. The wrapper is only ever rendered, so `LoopCube` runs only as
// `lazy` renders it. `local()` is the direct read beside the transported one.
const DefaultLoop = lazy(async () => {
  function LoopCube() {
    const [local] = createSignal(0);
    const [running, start, stop] = createLoop(frame);
    start();
    local();
    return <button onClick={stop}>{String(running())}</button>;
  }
  return { default: LoopCube };
});

// The read sits in a helper the loaded body calls: the call `kick()` is
// written directly in the body, so it is withdrawn from the refusal, and the
// read is reported there as it is for a component rendered by its tag.
const NestedLoop = lazy(async () => {
  function NestedCube() {
    const [running, start] = createLoop(frame);
    const kick = () => start();
    kick();
    return <div>{String(running())}</div>;
  }
  return { default: NestedCube };
});

export function DefaultPage() {
  return (
    <main>
      <DefaultLoop />
      <NestedLoop />
    </main>
  );
}

// A wildcard member write, as app-game holds one: it cannot reach the fresh
// tuple or the fresh module object (ADR 0288).
export function assignAll<T extends object>(target: T, source: object): T {
  return Object.assign(target, source);
}
