import { getOwner, onCleanup, runWithOwner } from "solid-js";
import { createReducer } from "@solid-primitives/memo";
export default function App() {
  const owner = getOwner();
  const [state, dispatch] = createReducer((value: number, by: number) => {
    onCleanup(() => {});
    return value + by;
  }, 0);
  const finish = () => {
    runWithOwner(owner, () => { dispatch(1); });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={finish}>dispatch</button><p>{state()}</p><p id="done">waiting</p></>;
}
