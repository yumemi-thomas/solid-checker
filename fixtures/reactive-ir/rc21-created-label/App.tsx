import { createEffect, flush } from "solid-js";

export function pick(): void {
  const label = document.createElement("label");
  label.htmlFor = "x";
  document.body.appendChild(label);
  label.click();
  label.remove();
}

export function App() {
  createEffect(() => 0, () => {
    pick();
  });
  return <input id="x" onClick={() => { flush(); }} />;
}
