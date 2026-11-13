import "fz-pub";
import { winStep } from "fz-win";
function Wrap() {
  winStep(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
