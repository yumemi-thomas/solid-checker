import { typedStep } from "fz-typed";
function Wrap() {
  typedStep(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
