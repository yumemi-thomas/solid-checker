import { createContext, useContext } from "solid-js";

const Ctx = createContext<number>();

export function Page() {
  const download = () => {
    const anchor = document.createElement("a");
    document.body.append(anchor);
    anchor.click();
    anchor.remove();
  };
  download();
  return (
    <div>
      <button onClick={() => { useContext(Ctx); }}>go</button>
    </div>
  );
}
