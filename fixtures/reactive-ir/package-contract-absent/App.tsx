// package-contract-missing (SC9005): uncontracted-package resolves -- it has
// a package.json and type declarations under node_modules -- and its manifest
// declares a solid-js peer dependency, so it participates in Solid
// reactivity. But it ships no solid-reactivity.json and the project declares
// no local override, so the checker cannot rely on its export summaries and
// says so once, at the import. The contracted sibling of this project is
// fixtures/reactive-ir/package-consumer, whose package ships a verified
// contract and reports nothing.
import { readCount } from "uncontracted-package";

export function App() {
  return <div>{readCount()}</div>;
}
