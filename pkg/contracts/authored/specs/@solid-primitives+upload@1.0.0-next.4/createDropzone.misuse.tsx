import { createDropzone } from "@solid-primitives/upload";
export default function App() {
  const zone = createDropzone();
  const current = zone.files().length;
  return <p>{String(current)}</p>;
}
