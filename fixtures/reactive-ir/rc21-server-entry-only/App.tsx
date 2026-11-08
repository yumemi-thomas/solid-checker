import { createSignal } from "solid-js";
import { renderToStream } from "@solidjs/web";

export function App() {
  const [count] = createSignal(0);
  const value = count();
  return <p>{value}</p>;
}

export const out = () => renderToStream(() => <App />);
