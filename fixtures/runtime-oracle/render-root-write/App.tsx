import { createSignal } from "solid-js";

// jibe-run Home (src/routes/index.tsx): exported default, body writes inside try/catch.
// The caller is another file, mount.tsx: render(() => Home()), so the write runs under
// the root render creates (@solidjs/web rc.13 dist/web.dev.js:488-495).
export default function Home() {
  const [selectedStarter, setSelectedStarter] = createSignal("blank");
  try {
    setSelectedStarter("react");
  } catch {
    /* plain gallery */
  }
  return <span id="starter">{selectedStarter()}</span>;
}

// Called only from hydrate's code callback (mount.tsx); hydrate reaches render on every path.
const [hydrated, setHydrated] = createSignal("no");
export function markHydrated() {
  setHydrated("yes");
}

// Control: the write sits in a click handler, which is legal wherever the click runs.
export function Control() {
  const [label, setLabel] = createSignal("idle");
  return (
    <button id="control" onClick={() => setLabel("clicked")}>
      {label()}
    </button>
  );
}

void import("./mount");
