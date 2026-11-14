import { clock } from "fz-get";
function Wrap() {
  void clock.now;
  return null;
}
export function App() {
  return <Wrap />;
}
