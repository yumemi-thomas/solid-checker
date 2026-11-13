import { vendorStep } from "./vendor/step";
function Wrap() {
  vendorStep(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
