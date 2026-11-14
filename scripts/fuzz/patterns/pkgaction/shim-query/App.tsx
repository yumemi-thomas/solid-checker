import { vendorStep } from "./step.vendor.js?fz";
function Wrap() {
  vendorStep(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
