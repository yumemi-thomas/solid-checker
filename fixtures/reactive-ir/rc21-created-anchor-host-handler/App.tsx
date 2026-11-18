import { createContext, createSignal, useContext } from "solid-js";

const Ctx = createContext<number>();

export function Page() {
  const [count, setCount] = createSignal(0);
  const download = () => {
    const anchor = document.createElement("a");
    document.querySelector("main")!.append(anchor);
    anchor.click();
    anchor.remove();
  };
  download();
  return (
    <div>
      <button onClick={() => { useContext(Ctx); }}>go</button>
      <button onClick={() => setCount(count() + 1)}>{count()}</button>
    </div>
  );
}
