import { getOwner, onCleanup, runWithOwner } from "solid-js";
import { createReducer } from "@solid-primitives/memo";
export default function App() {
  const owner = getOwner();
  const [state, dispatch] = createReducer((value: number, by: number) => value + by, 0, {
    equals: (prev, next) => {
      runWithOwner(owner, () => { onCleanup(() => {}); });
      return prev === next;
    }
  });
  const finish = () => {
    runWithOwner(null, () => { dispatch(1); });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={finish}>compare</button><p>{state()}</p><p id="done">waiting</p></>;
}
