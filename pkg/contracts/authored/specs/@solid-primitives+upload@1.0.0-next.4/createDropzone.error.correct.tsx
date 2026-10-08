import { createDropzone } from "@solid-primitives/upload";
export default function App() {
  const zone = createDropzone();
  return <p>{String(zone.error())}</p>;
}
