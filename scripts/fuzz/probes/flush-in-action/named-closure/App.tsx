// ADR 0479: flush() in a named closure called once in an action step throws FLUSH_IN_ACTION (rc.8+ dev).
import { action, flush } from "solid-js";

const save = action(function* () {
  const reveal = () => flush();
  reveal();
  yield 1;
});

export default function App() {
  return <button onClick={() => void save()}>save</button>;
}
