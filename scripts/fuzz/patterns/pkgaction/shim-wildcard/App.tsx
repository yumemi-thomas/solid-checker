import { vendorStep } from "./step.vendor.js";
function Wrap() {
  vendorStep(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
