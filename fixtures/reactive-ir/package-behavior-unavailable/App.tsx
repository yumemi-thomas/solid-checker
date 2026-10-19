// package-behavior-unavailable (SC9005): uncontracted-package resolves -- it has
// a package.json and type declarations under node_modules -- and its manifest
// declares a solid-js peer dependency, so it participates in Solid
// reactivity. But the project declares no runtime host and ships no analyzable
// source for it, so the checker cannot establish what readCount does and says
// so once, at the import.
import { readCount } from "uncontracted-package";

export function App() {
  return <div>{readCount()}</div>;
}
