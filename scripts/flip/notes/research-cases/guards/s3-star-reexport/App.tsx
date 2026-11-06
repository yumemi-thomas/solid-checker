import { createSignal } from "./re";
export function App() {
  const [a] = createSignal(1); const x = a();
  return <div><p>{a()}{x}</p></div>;
}
