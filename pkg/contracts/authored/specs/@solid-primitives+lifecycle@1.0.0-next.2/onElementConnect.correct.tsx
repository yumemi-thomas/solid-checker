import { onElementConnect } from "@solid-primitives/lifecycle";
export default function App() {
  const element = document.createElement("div");
  onElementConnect(element, () => {});
  return <p>ready</p>;
}
