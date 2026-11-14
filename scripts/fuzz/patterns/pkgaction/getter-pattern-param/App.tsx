import { clock, Clock } from "fz-get";
void Clock;
function Wrap() {
  (({ now = 0 }: { now: number }) => now)(clock);
  return null;
}
export function App() {
  return <Wrap />;
}
