import { onElementConnect } from "@solid-primitives/lifecycle";
const element = document.createElement("div");
onElementConnect(element, () => {}); // Expected NO_OWNER_CLEANUP (detached branch).
export default function App() { return <p>ready</p>; }
