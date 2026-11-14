import { vendorStep } from "./re";
function Wrap() {
  vendorStep(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
