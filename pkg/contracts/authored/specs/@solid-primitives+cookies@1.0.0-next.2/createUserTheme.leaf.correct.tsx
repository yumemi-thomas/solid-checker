/** @jsxImportSource @solidjs/web */
import { createUserTheme } from "@solid-primitives/cookies";
export default function App() {
  createUserTheme("research-theme");
  return <p>ready</p>;
}
