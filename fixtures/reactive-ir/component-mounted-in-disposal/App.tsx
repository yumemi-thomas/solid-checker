import { action, createRoot, createSignal, onCleanup, onFlush } from "solid-js";

const [, setW] = createSignal(0);

function OnlyInCleanup() {
  setW(1);
  return <p />;
}

function AlsoOutside() {
  setW(2);
  return <p />;
}

function InHandler() {
  setW(3);
  return <p />;
}

// `action` and `onFlush` test the owner alone, never the disposal depth: a
// component only a cleanup mounts calls them in an owned scope (Chrome).
const act = action(function* () {
  yield 1;
});
function ActionOnlyInCleanup() {
  void act();
  return <p />;
}
function FlushOnlyInCleanup() {
  onFlush(() => {});
  return <p />;
}

export function Exported() {
  setW(4);
  return <p />;
}

export function App() {
  const dispose = createRoot((d) => {
    onCleanup(() => {
      createRoot(() => <OnlyInCleanup />);
      createRoot(() => <AlsoOutside />);
      createRoot(() => <Exported />);
      createRoot(() => <ActionOnlyInCleanup />);
      createRoot(() => <FlushOnlyInCleanup />);
    });
    return d;
  });
  createRoot(() => <AlsoOutside />);
  return <button onClick={() => { createRoot(() => <InHandler />); dispose(); }} />;
}
