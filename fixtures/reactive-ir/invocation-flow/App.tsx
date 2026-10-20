import { createEffect, createSignal } from "solid-js";
import { makeNavigation, retain, retainWithLog, type NavigationOptions } from "./navigation";
import { HazardTree, Tree } from "./tree";

// ---- Clean: every invocation of the callback is one event dispatch.

// The options callback runs from the keydown listener of the returned row
// props, spread onto an intrinsic element.
export function SpreadHandlers() {
  const [count] = createSignal(0);
  const nav = makeNavigation({
    onActivate: (id) => console.log(id, count()),
    scrollTo: () => undefined,
  });
  return <div {...nav.rowProps("a")}>row</div>;
}

// The click listener comes back from `rowProps` and reaches the row element
// through three forwarding components, one of them recursive.
export function ForwardedRows() {
  const [count] = createSignal(0);
  return (
    <Tree ids={["a"]} rowProps={(id) => ({ onClick: () => console.log(id, count()) })}>
      {(id) => <span>{id}</span>}
    </Tree>
  );
}

// The retained closure runs only in a click handler.
export function ClickRetained() {
  const [count] = createSignal(0);
  const run = retain({ onRun: () => console.log(count()) });
  return <button onClick={() => run()}>run</button>;
}

// ---- Uncertifiable: some invocation is not proven to be that one role.

export function ThisSibling() {
  const [count] = createSignal(0);
  const nav = makeNavigation({
    onActivate: (id) => console.log(id, count()),
    scrollTo(this: NavigationOptions, id: string) {
      this.onActivate?.(id);
    },
  });
  return <div {...nav.rowProps("a")}>row</div>;
}

export function AppliedRetained() {
  const [count] = createSignal(0);
  const run = retain({ onRun: () => console.log(count()) });
  createEffect(
    () => 1,
    () => run(),
  );
  return <main>applied</main>;
}

export function MixedRetained() {
  const [count] = createSignal(0);
  const run = retain({ onRun: () => console.log(count()) });
  setTimeout(() => run(), 0);
  return <button onClick={() => run()}>run</button>;
}

export function BodyRetained() {
  const [count] = createSignal(0);
  const run = retain({ onRun: () => console.log(count()) });
  run();
  return <main>body</main>;
}

export function AbsentSibling() {
  const [count] = createSignal(0);
  const run = retainWithLog({ onRun: () => console.log(count()) });
  return <button onClick={() => run()}>run</button>;
}

export function AsyncRetained() {
  const [count] = createSignal(0);
  const run = retain({ onRun: () => console.log(count()) });
  async function later() {
    await Promise.resolve();
    run();
  }
  return <button onClick={() => void later()}>run</button>;
}

export function ReassignedRetained() {
  const [count] = createSignal(0);
  let run = retain({ onRun: () => console.log(count()) });
  const replace = () => {
    run = () => undefined;
  };
  return (
    <button
      onClick={() => {
        replace();
        run();
      }}
    >
      run
    </button>
  );
}

export function OpaqueRetained() {
  const [count] = createSignal(0);
  const run = retain({ onRun: () => console.log(count()) });
  opaque(run);
  return <main>opaque</main>;
}

export function HazardRows() {
  const [count] = createSignal(0);
  return <HazardTree id="a" depth={0} rowProps={(id) => ({ onClick: () => console.log(id, count()) })} />;
}
