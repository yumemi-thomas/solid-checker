import * as ns from "./barrel";
export const handedOut = ns.escapes;
export function App() {
  const result = (ns.describe as typeof ns.describe)({ label() { return "fixed"; } });
  return <div>{result}</div>;
}
