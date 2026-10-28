// ADR 0480: a signal read through a tuple holder's slot in a component body is a strict read (STRICT_READ_UNTRACKED).
import { createSignal } from "solid-js";

export default function App() {
  const [count] = createSignal(1);
  const pair = [0, count] as const;
  const [, read] = pair;
  const seen = read();
  return <span id="w">{seen}</span>;
}
