import { action, flush } from "solid-js";

// Each layer below runs where no call edge places it; the action body reaches
// it, so a step is on the stack (Chrome, rc.21: FLUSH_IN_ACTION). Each is an
// SC2006 obligation, blocker `function-run-unplaced`.

const viaObject = { go() { flush(); } };

class Viewer {
  go() { flush(); }
  get label() { flush(); return "x"; }
}
const viewer = new Viewer();

const viaMap = new Map([["go", () => { flush(); }]]);

function runCallback(callback: () => void) { callback(); }
function handed() { flush(); }

const coerced = { toString() { flush(); return "x"; } };

function tag(strings: TemplateStringsArray) { flush(); return strings[0]; }

export const save = action(function* () {
  viaObject.go();
  viewer.go();
  void viewer.label;
  viaMap.get("go")!();
  runCallback(handed);
  void `${coerced}`;
  void tag`x`;
  yield;
});

// The proven position stays a violation.
export const direct = action(function* () {
  flush();
});

// Not reached by anything: silent.
function neverCalled() {
  const unused = { go() { flush(); } };
  return unused;
}

export function App() {
  return <button id="b" onClick={() => { save(); }}>x</button>;
}
