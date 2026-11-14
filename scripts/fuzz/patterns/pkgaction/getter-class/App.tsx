import { Clock } from "fz-get";
const clock = new Clock();
function Wrap() {
  void clock.now;
  return null;
}
export function App() {
  return <Wrap />;
}
