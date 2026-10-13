import { createReducer } from "@solid-primitives/memo";

export default function App() {
  const [state] = createReducer((value: number, by: number) => value + by, 0);
  const current = state();
  return <p>{String(current)}</p>;
}
