import { onCleanup } from "solid-js";
import { click } from "./delivery";

export function App() {
  const element = <button onClick={() => onCleanup(() => {})} /> as HTMLButtonElement;
  click(element);
  return element;
}
