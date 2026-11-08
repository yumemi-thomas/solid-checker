// The project names dep-a only in a re-export: no import declaration loads it.
export { useBox } from "dep-a";

export function App() {
  return <p>static</p>;
}
