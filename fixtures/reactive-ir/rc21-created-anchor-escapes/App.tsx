import { createSignal } from "solid-js";

function place(element: HTMLElement) {
  document.querySelector("main")?.append(element);
}

function download(url: string) {
  const anchor = document.createElement("a");
  anchor.href = url;
  place(anchor);
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
