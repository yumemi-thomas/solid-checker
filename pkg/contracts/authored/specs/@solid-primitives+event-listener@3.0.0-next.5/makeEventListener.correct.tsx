import { makeEventListener } from "@solid-primitives/event-listener";
export default function App() {
  makeEventListener(window, "click", () => {});
  return <p>candidate</p>;
}
