import { Solid } from "./re";
export function App() {
  const [a] = Solid.createSignal(1); const x = a();
  return <div><p>{a()}{x}</p></div>;
}
