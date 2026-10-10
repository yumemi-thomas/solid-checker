import { createPrefersDark } from "@solid-primitives/media";

export default function App() {
  const dark = createPrefersDark();
  return <p>{String(dark())}</p>;
}
