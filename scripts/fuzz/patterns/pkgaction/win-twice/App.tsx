import "fz-pub";
import { twiceStep } from "fz-win";
function Wrap() {
  twiceStep(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
