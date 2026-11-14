import { clock, Clock } from "fz-get";
void Clock;
function Wrap() {
  const { now = 0 } = clock;
  void now;
  return null;
}
export function App() {
  return <Wrap />;
}
