import { createDropzone } from "@solid-primitives/upload";
export default function App() {
  const zone = createDropzone();
  const current = zone.isLoading();
  return <p>{String(current)}</p>;
}
