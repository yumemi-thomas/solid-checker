import { onSettled } from "solid-js";
import { createDropzone } from "@solid-primitives/upload";
export default function App() {
  createDropzone();
  onSettled(() => {});
  return <p>ready</p>;
}
