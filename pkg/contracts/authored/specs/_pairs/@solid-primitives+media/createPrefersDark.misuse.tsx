import { createPrefersDark } from "@solid-primitives/media";

export default function App() {
  const dark = createPrefersDark();
  const current = dark();
  return <p>{String(current)}</p>;
}
