/** @jsxImportSource @solidjs/web */
import { createUserTheme } from "@solid-primitives/cookies";
export default function App() {
  const [theme] = createUserTheme("research-theme");
  const current = theme();
  return <p>{String(current)}</p>;
}
