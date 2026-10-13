import { createMediaQuery } from "@solid-primitives/media";
export default function App() {
  const narrow = createMediaQuery("(max-width: 600px)");
  return <p>{String(narrow())}</p>;
}

