import { flush } from "solid-js";
import { runAction } from "fz-act";

// The project never names `action`: the dependency makes it (`runAction(cb) { return action(cb); }`), so the generator handed to
// it is the step body of an action no walk of the project proves (ADR 0961). The flush is an uncertifiable row (SC2006), never silence.
const save = runAction(function* () {
  flush();
  yield Promise.resolve();
});

export function App() {
  return <button onClick={() => void save()}>go</button>;
}
