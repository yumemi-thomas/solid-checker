// open-work: a direct read of a source before its own declarator; rc.17 throws ReferenceError, no Solid diagnostic.
import { createSignal } from "solid-js";

export default function App() {
  [1].forEach(() => r());
  const [r] = createSignal(0);
  return <span id="w">{r()}</span>;
}
