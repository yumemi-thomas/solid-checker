import { runStep } from "fz-step";
function Wrap() {
  setTimeout(() => {
    void runStep(() => {});
  });
  return null;
}
export function App() {
  return <Wrap />;
}
