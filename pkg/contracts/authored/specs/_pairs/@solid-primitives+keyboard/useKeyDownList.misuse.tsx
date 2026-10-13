import { useKeyDownList } from "@solid-primitives/keyboard";
export default function App() {
  const keys = useKeyDownList();
  const value = keys().length;
  return <p>{String(value)}</p>;
}
