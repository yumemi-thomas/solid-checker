import { WindowEventListener } from "@solid-primitives/event-listener";
export default function App() {
  WindowEventListener({ onClick: () => {} });
  return <p>candidate</p>;
}
