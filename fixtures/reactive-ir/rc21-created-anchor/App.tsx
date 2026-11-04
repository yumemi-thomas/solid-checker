import { createSignal } from "solid-js";

function download(url: string) {
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = "x";
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
}

export function Page() {
  const [count, setCount] = createSignal(0);
  download("/boot");
  return (
    <div>
      <button onClick={() => download("/x")}>download</button>
      <button onClick={() => setCount(count() + 1)}>{count()}</button>
    </div>
  );
}
