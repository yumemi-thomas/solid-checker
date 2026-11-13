import "fz-pub";
import { docStep } from "fz-win";
function Wrap() {
  docStep(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
