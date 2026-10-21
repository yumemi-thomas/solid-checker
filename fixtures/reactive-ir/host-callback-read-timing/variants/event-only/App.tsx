import { onCleanup } from "solid-js";
export function App() {
  return <button onClick={() => onCleanup(() => {})} />;
}
