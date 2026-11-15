import { location } from "./held";
export function App() {
  const s = location.search;
  return <i>{s}</i>;
}
