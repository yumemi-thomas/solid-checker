import { For, action, createTrackedEffect, resolve } from "solid-js";

// An action's first step runs on its caller's stack, under the caller's observer.
export function FromTracked() {
  const step = action(function* () {
    resolve(() => 1); // uncertifiable
  });
  createTrackedEffect(() => {
    step();
  });
  return <div />;
}

export function FromHandler() {
  const step = action(function* () {
    resolve(() => 1); // clean: a handler has no observer
  });
  return <button onClick={() => { step(); }}>go</button>;
}

export function PastYield() {
  const step = action(function* () {
    yield Promise.resolve(1);
    resolve(() => 1); // clean: a resumed step has no observer
  });
  createTrackedEffect(() => {
    step();
  });
  return <div />;
}

// A deferred role inside tracked JSX proves no clear observer.
export function InFlowJsx() {
  return <For each={[1]}>{() => <div>{(() => { resolve(() => 1); return 1; })()}</div>}</For>;
}

export function FlowHandler() {
  return <For each={[1]}>{() => <button onClick={() => { resolve(() => 1); }}>go</button>}</For>;
}
