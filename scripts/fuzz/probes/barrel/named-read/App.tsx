import { createSignal } from "./helpers";

export default function App() {
  const [r] = createSignal(1);
  const seen = r();
  return <span id="w">{seen}</span>;
}
