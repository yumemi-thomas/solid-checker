// ADR 0478: flush() in a helper's callback, called in an action step, throws FLUSH_IN_ACTION (rc.8+ dev).
import { action, flush } from "solid-js";

function run(cb: () => unknown) {
  cb();
  return 1;
}

const save = action(function* () {
  run(() => flush());
  yield 1;
});

export default function App() {
  return <button onClick={() => void save()}>save</button>;
}
