import { useKeyDownList } from "@solid-primitives/keyboard";
export default function App() {
  const keys = useKeyDownList();
  return <p>{String(keys().length)}</p>;
}
