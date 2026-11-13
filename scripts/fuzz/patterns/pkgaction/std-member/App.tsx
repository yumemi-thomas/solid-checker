import { listener } from "fz-typed";
function Wrap() {
  listener.handleEvent(new Event("x"));
  return null;
}
export function App() {
  return <Wrap />;
}
