import { clock, Clock } from "fz-get";
void Clock;
function Wrap() {
  let n = 0;
  ({ now: n } = clock);
  void n;
  return null;
}
export function App() {
  return <Wrap />;
}
