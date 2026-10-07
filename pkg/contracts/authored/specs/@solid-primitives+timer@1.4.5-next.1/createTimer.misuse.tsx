import { createTimer } from "@solid-primitives/timer";
export default function App() {
  const copy = async () => {
    await Promise.resolve();
    createTimer(() => {}, 10, setTimeout);
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void copy()}>Copy</button><p id="done">waiting</p></>;
}

