import { createRoot, createSignal, onCleanup } from "solid-js";

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
    });
    return d;
  });
  createRoot(() => <AlsoOutside />);
  return <button onClick={() => { createRoot(() => <InHandler />); dispose(); }} />;
}
