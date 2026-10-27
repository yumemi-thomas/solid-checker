import { createSignal } from "solid-js";

const [, setN] = createSignal(0);

// Violation: a tag renders `Tagged`, so its body is a component body.
function Tagged() {
  setN(1);
  return <li />;
}

export function App() {
  return <Tagged />;
}

// Violation: an exported capitalized function returning JSX is the host's
// component by module surface (ADR 0331), though nothing here tags it.
export function Page() {
  setN(2);
  return <li />;
}
