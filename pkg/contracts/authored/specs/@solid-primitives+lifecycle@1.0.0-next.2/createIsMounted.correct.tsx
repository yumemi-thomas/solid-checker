import { createIsMounted } from "@solid-primitives/lifecycle";
export default function App() {
  const mounted = createIsMounted();
  return <p>{String(mounted())}</p>;
}
