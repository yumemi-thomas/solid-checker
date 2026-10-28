// ADR 0478: flush() in an IIFE in an action step throws FLUSH_IN_ACTION (rc.8+ dev).
import { action, flush } from "solid-js";

const save = action(function* () {
  (() => flush())();
  yield 1;
});

export default function App() {
  return <button onClick={() => void save()}>save</button>;
}
