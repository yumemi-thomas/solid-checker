import { createRoot } from "solid-js";
import { createDropzone } from "@solid-primitives/upload";
createRoot(dispose => {
  createDropzone();
  queueMicrotask(dispose);
});
export default function App() { return <p>ready</p>; }
