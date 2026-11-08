import { createEffect, flush } from "solid-js";

addEventListener("click", () => {
  flush();
});
const listen = document.addEventListener;
listen.call(document, "click", () => {
  flush();
});
document["addEventListener"]("click", () => {
  flush();
});

export function download(): void {
  const anchor = document.createElement("a");
  anchor.href = "#x";
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
}

export function App() {
  createEffect(() => 0, () => {
    download();
  });
  return <i />;
}
