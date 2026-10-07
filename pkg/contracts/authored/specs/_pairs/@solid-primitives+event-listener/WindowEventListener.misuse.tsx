import { onSettled } from "solid-js";
import { WindowEventListener } from "@solid-primitives/event-listener";
export default function App() {
  onSettled(() => { WindowEventListener({ onClick: () => {} }); });
  return <p>candidate</p>;
}
