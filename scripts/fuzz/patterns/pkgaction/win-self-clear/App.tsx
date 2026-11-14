import { clearStep } from "fz-win";
function Wrap() {
  clearStep(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
