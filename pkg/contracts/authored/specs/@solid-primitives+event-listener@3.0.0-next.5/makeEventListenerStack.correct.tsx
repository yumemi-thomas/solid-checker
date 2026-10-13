import { makeEventListenerStack } from "@solid-primitives/event-listener";
export default function App() {
  makeEventListenerStack(window);
  return <p>candidate</p>;
}
