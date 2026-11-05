import { flush } from "solid-js";

// No callback in this program runs as an action's steps, so no flush() can be
// inside one: the unplaced layers below stay silent for SC2006.

const viaObject = { go() { flush(); } };

class Viewer {
  go() { flush(); }
}
const viewer = new Viewer();

const viaMap = new Map([["go", () => { flush(); }]]);

export function App() {
  return (
    <button id="b" onClick={() => { viaObject.go(); viewer.go(); viaMap.get("go")!(); }}>x</button>
  );
}
