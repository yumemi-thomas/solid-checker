import { onSettled } from "solid-js";
import { createPrefersDark } from "@solid-primitives/media";
export default function App() {
  createPrefersDark();
  return <p>ready</p>;
}
