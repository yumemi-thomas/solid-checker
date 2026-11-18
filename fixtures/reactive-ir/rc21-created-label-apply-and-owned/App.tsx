import { createContext, createEffect, getOwner, runWithOwner, useContext } from "solid-js";

const Ctx = createContext<number>();

export function Page() {
  const owner = getOwner();
  const pick = () => {
    const label = document.createElement("label");
    label.htmlFor = "x";
    document.body.append(label);
    label.click();
    label.remove();
  };
  createEffect(() => 0, () => {
    pick();
  });
  return (
    <div>
      <button id="x" onClick={() => { useContext(Ctx); }}>go</button>
      <button onClick={() => runWithOwner(owner, pick)}>owned</button>
    </div>
  );
}
