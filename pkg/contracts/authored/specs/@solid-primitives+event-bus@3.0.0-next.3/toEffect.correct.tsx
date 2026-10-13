import { toEffect } from "@solid-primitives/event-bus";
export default function App() {
  toEffect<string>(() => {});
  return <p>ready</p>;
}
