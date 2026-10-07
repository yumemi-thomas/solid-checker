import { createMediaQuery } from "@solid-primitives/media";
export default function App() {
  createMediaQuery("(min-width: 1px)");
  return <p>candidate</p>;
}
