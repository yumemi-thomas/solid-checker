import { clock, Clock } from "fz-get";
void Clock;
function Wrap() {
  let now = 0;
  ({ now } = clock);
  void now;
  return null;
}
export function App() {
  return <Wrap />;
}
