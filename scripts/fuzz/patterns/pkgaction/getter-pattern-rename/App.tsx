import { clock, Clock } from "fz-get";
void Clock;
function Wrap() {
  const { now: n } = clock;
  void n;
  return null;
}
export function App() {
  return <Wrap />;
}
