import { makeClock } from "fz-get";
const made = makeClock();
function Wrap() {
  const { now } = made;
  return null;
}
export function App() {
  return <Wrap />;
}
