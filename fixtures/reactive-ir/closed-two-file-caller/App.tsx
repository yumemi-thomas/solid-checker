import { placedUse, wrapUse } from "./helper";

const kept: (() => void)[] = [];
function hold(f: () => void) {
  kept.push(f);
}
export function App() {
  wrapUse();
  placedUse();
  hold(() => wrapUse());
  return <i />;
}
