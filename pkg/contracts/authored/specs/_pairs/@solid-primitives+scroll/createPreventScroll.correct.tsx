import { createPreventScroll } from "@solid-primitives/scroll";
export default function App() {
  createPreventScroll({ enabled: false });
  return <p>candidate</p>;
}
