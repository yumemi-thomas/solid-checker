import { createEffect, flush } from "solid-js";

document.addEventListener("change", () => {
  flush();
});

export function pick(): void {
  const box = document.createElement("input");
  box.type = "checkbox";
  document.body.appendChild(box);
  box.click();
  box.remove();
}

export function App() {
  createEffect(() => 0, () => {
    pick();
  });
  return <i />;
}
